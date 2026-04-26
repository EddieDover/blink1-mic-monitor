use anyhow::{anyhow, Context, Result};
use blinkrs::{Blinkers, Message, Color};
use clap::{Parser, ValueEnum};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Sample;
use crossbeam_channel::{bounded, Sender};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::Duration;
use tao::event_loop::{ControlFlow, EventLoop};
use tray_icon::{TrayIconBuilder, menu::{Menu, MenuItem, MenuEvent}};

const DEFAULT_MUTED_COLOR: &str = "#ff0000";
const MUTE_VALUE: f32 = -100.0;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// List available input devices and exit
    #[arg(long)]
    list_devices: bool,

    /// Input device index (default: system default)
    #[arg(short, long)]
    device: Option<usize>,

    /// Output method
    #[arg(short, long, value_enum, default_value_t = OutputMethod::Blink1)]
    output: OutputMethod,

    /// Show live audio level meter
    #[arg(short, long)]
    verbose: bool,

    /// Blink(1) color when muted
    #[arg(long, default_value = DEFAULT_MUTED_COLOR)]
    muted_color: String,

    /// Blink(1) color when unmuted
    #[arg(long)]
    unmuted_color: Option<String>,
}

#[derive(Clone, ValueEnum, PartialEq)]
enum OutputMethod {
    Console,
    Blink1,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.list_devices {
        list_devices()?;
        return Ok(());
    }

    let host = cpal::default_host();
    
    let device = if let Some(index) = cli.device {
        let mut devices = host.input_devices().context("Failed to get input devices")?;
        devices.nth(index).ok_or_else(|| anyhow!("Device index {} not found", index))?
    } else {
        host.default_input_device().ok_or_else(|| anyhow!("No default input device available"))?
    };

    #[allow(deprecated)]
    let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
    println!("Monitoring: {}", device_name);

    let config: cpal::StreamConfig = device.default_input_config()
        .context("Failed to get default input config")?
        .into();

    let (tx, rx) = bounded::<f32>(10);

    let verbose = cli.verbose;

    let callback_tx = tx.clone();
    let err_fn = move |err| {
        eprintln!("an error occurred on stream: {}", err);
    };

    let stream = match device.default_input_config()?.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config,
            move |data: &[f32], _: &_| write_input_data(data, &callback_tx),
            err_fn,
            None
        )?,
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config,
            move |data: &[i16], _: &_| write_input_data(data, &callback_tx),
            err_fn,
            None
        )?,
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config,
            move |data: &[u16], _: &_| write_input_data(data, &callback_tx),
            err_fn,
            None
        )?,
        sample_format => return Err(anyhow!("Unsupported sample format '{:?}'", sample_format)),
    };

    stream.play()?;
    println!("Press Ctrl+C to stop\n");

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    let r_ctrlc = running.clone();
    ctrlc::set_handler(move || {
        println!("\nReceived Ctrl+C, shutting down...");
        r_ctrlc.store(false, Ordering::SeqCst);
        thread::sleep(Duration::from_millis(500));
        std::process::exit(0);
    })?;

    let r_thread = running.clone();
    let thread_verbose = verbose;
    
    let output_method = cli.output.clone();
    let muted_color = cli.muted_color.clone();
    let unmuted_color = cli.unmuted_color.clone();

    let monitor_thread = thread::spawn(move || {
        let mut output: Box<dyn StatusOutput> = match output_method {
            OutputMethod::Console => Box::new(ConsoleOutput::new()),
            OutputMethod::Blink1 => {
                println!("Initializing Blink(1) in background thread...");
                match Blink1Output::new(&muted_color, unmuted_color.as_deref()) {
                    Ok(blink1) => {
                        println!("Blink(1) device initialized successfully in thread.");
                        Box::new(blink1)
                    },
                    Err(e) => {
                        eprintln!("Warning: Failed to initialize Blink(1): {}", e);
                        eprintln!("Falling back to Console output.");
                        Box::new(ConsoleOutput::new())
                    }
                }
            }
        };

        output.set_muted();
        let mut is_muted = true;

        while r_thread.load(Ordering::SeqCst) {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(db) => {
                    if thread_verbose {
                        print_level(db);
                    }

                    if db == MUTE_VALUE {
                        if !is_muted {
                            is_muted = true;
                            if thread_verbose { println!(); }
                            output.set_muted();
                        }
                    } else {
                        if is_muted {
                            is_muted = false;
                            if thread_verbose { println!(); }
                            output.set_unmuted();
                        }
                    }
                }
                Err(_) => {
                    // Timeout or disconnected, just continue checking running flag
                }
            }
        }
        if thread_verbose {
            println!();
        }
        output.cleanup();
    });

    let event_loop = EventLoop::new();

    let tray_menu = Menu::new();
    let quit_i = MenuItem::new("Exit", true, None);
    tray_menu.append(&quit_i)?;

    let mut tray_icon = Some(
        TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("Mic Monitor")
            .build()?
    );

    let icon_rgba = vec![255u8, 0, 0, 255]; // Red
    let icon = tray_icon::Icon::from_rgba(icon_rgba, 1, 1).ok();
    if let Some(tray) = tray_icon.as_mut() {
        if let Some(i) = icon {
            let _ = tray.set_icon(Some(i));
        }
    }

    let menu_channel = MenuEvent::receiver();

    let mut monitor_thread_handle = Some(monitor_thread);

    event_loop.run(move |_event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(std::time::Instant::now() + std::time::Duration::from_millis(100));

        if let Ok(event) = menu_channel.try_recv() {
            if event.id == quit_i.id() {
                println!("Exiting...");
                r.store(false, Ordering::SeqCst);
                
                // Wait for the monitor thread to cleanup and finish
                if let Some(handle) = monitor_thread_handle.take() {
                    let _ = handle.join();
                }
                
                *control_flow = ControlFlow::Exit;
            }
        }
        
        // Check if monitor thread died/finished unexpectedly
        if let Some(handle) = monitor_thread_handle.as_ref() {
            if handle.is_finished() {
                 *control_flow = ControlFlow::Exit;
            }
        }
    });

}

fn write_input_data<T>(input: &[T], tx: &Sender<f32>)
where
    T: cpal::Sample,
    f32: cpal::FromSample<T>,
{
    let mut sum_sq = 0.0;
    let len = input.len() as f32;
    
    if len == 0.0 {
        return;
    }

    for &sample in input {
        let val: f32 = f32::from_sample(sample);
        sum_sq += val * val;
    }

    let rms = (sum_sq / len).sqrt();
    let db = if rms > 0.0 {
        20.0 * rms.log10()
    } else {
        MUTE_VALUE
    };

    let _ = tx.try_send(db);
}

fn print_level(db: f32) {
    let bar_len = ((db + 60.0) / 2.0).round() as i32;
    let bar_len = bar_len.clamp(0, 30) as usize;
    let bar = "#".repeat(bar_len) + &"-".repeat(30 - bar_len);
    print!("\r  Level: [{}] {:6.1} dB", bar, db);
    use std::io::{self, Write};
    io::stdout().flush().unwrap();
}

#[allow(deprecated)]
fn list_devices() -> Result<()> {
    let host = cpal::default_host();
    let devices = host.input_devices()?;
    let default_in = host.default_input_device().map(|d| d.name().unwrap_or_default());

    println!("Available input devices:\n");
    for (i, device) in devices.enumerate() {
        let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        let is_default = default_in.as_ref().map(|d| d == &name).unwrap_or(false);
        let marker = if is_default { "→" } else { " " };
        let label = if is_default { " (default)" } else { "" };
        println!("  {} [{}] {}{}", marker, i, name, label);
    }
    println!();
    Ok(())
}

// Outputs

trait StatusOutput: Send {
    fn set_muted(&mut self);
    fn set_unmuted(&mut self);
    fn cleanup(&mut self);
}

struct ConsoleOutput {
    is_muted: Option<bool>,
}

impl ConsoleOutput {
    fn new() -> Self {
        Self { is_muted: None }
    }
}

impl StatusOutput for ConsoleOutput {
    fn set_muted(&mut self) {
        if self.is_muted != Some(true) {
            self.is_muted = Some(true);
            println!("MUTED");
        }
    }

    fn set_unmuted(&mut self) {
        if self.is_muted != Some(false) {
            self.is_muted = Some(false);
            println!("UNMUTED");
        }
    }

    fn cleanup(&mut self) {
        println!("Shutting down...");
    }
}

struct Blink1Output {
    device: Blinkers,
    muted_color: Color,
    unmuted_color: Color,
    is_muted: Option<bool>,
}

impl Blink1Output {
    fn new(muted_hex: &str, unmuted_hex: Option<&str>) -> Result<Self> {
        let device = Blinkers::new().map_err(|e| anyhow!("Failed to connect to Blink(1): {}", e))?;
        
        let muted_color = parse_color(muted_hex)?;
        let unmuted_color = if let Some(hex) = unmuted_hex {
            parse_color(hex)?
        } else {
            Color::Three(0, 0, 0)
        };

        Ok(Self {
            device,
            muted_color,
            unmuted_color,
            is_muted: None,
        })
    }
}

impl StatusOutput for Blink1Output {
    fn set_muted(&mut self) {
        if self.is_muted != Some(true) {
            self.is_muted = Some(true);
            println!("Blink1: Setting to MUTED ({:?})", self.muted_color);
            if let Err(e) = self.device.send(Message::Fade(self.muted_color, Duration::from_millis(300), None)) {
                eprintln!("Failed to send mute command to Blink1: {}", e);
            }
        }
    }

    fn set_unmuted(&mut self) {
        if self.is_muted != Some(false) {
            self.is_muted = Some(false);
            println!("Blink1: Setting to UNMUTED ({:?})", self.unmuted_color);
            if let Err(e) = self.device.send(Message::Fade(self.unmuted_color, Duration::from_millis(300), None)) {
                eprintln!("Failed to send unmute command to Blink1: {}", e);
            }
        }
    }

    fn cleanup(&mut self) {
         if let Err(e) = self.device.send(Message::Fade(Color::Three(0, 0, 0), Duration::from_millis(100), None)) {
             eprintln!("Failed to turn off Blink1: {}", e);
         }
    }
}

impl Drop for Blink1Output {
    fn drop(&mut self) {
        Self::cleanup(self);
    }
}

fn parse_color(hex: &str) -> Result<Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Err(anyhow!("Invalid color format. Use #RRGGBB"));
    }
    let r = u8::from_str_radix(&hex[0..2], 16)?;
    let g = u8::from_str_radix(&hex[2..4], 16)?;
    let b = u8::from_str_radix(&hex[4..6], 16)?;
    Ok(Color::Three(r, g, b))
}
