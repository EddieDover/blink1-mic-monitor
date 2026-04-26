# Mic Mute Status Monitor

Monitors microphone mute status from the selected audio input and reflects that state to a [Blink(1)](https://blink1.thingm.com/) RGB device or the console. Includes a system tray icon for easy exit.

## Requirements

- Rust toolchain (install via [rustup](https://rustup.rs/))
- A [Blink(1)](https://blink1.thingm.com/) USB device (optional — console output mode works without one)
- System packages: cargo, libxdo development headers, ALSA, libusb, GTK3, libappindicator-gtk3

Package name mapping for the libxdo dependency:
- Fedora/RHEL: `libxdo-devel`
- Debian/Ubuntu: `libxdo-dev`
- Arch: `xdotool` (provides libxdo)

## Installation

The `install.sh` script handles dependency installation, building, and deploying the binary:

```bash
./install.sh
```

This will:
1. Optionally install required build/system dependencies for your distro (Fedora/RHEL, Debian/Ubuntu, or Arch Linux)
2. Build the release binary via `cargo build --release`
3. Install the binary to `~/.local/bin/blink1-mic-monitor`
4. Install a desktop entry to `~/.local/share/applications/`
5. Create a udev rule at `/etc/udev/rules.d/51-blink1.rules` for non-root USB access to the Blink(1) device

To uninstall:

```bash
./uninstall.sh
```

### Manual Build

```bash
cargo build --release
# Binary will be at target/release/blink1-mic-monitor
```

## Usage

List available input devices:

```bash
blink1-mic-monitor --list-devices
```

Run with the default Blink(1) output (monitors the system default microphone):

```bash
blink1-mic-monitor
```

Run with console output instead of Blink(1):

```bash
blink1-mic-monitor -o console
```

Use a specific input device by index:

```bash
blink1-mic-monitor -d 2
```

Show a live audio level meter in the terminal:

```bash
blink1-mic-monitor -v
```

Set a custom muted color (hex):

```bash
blink1-mic-monitor --muted-color "#ff0000"
```

Set a custom unmuted color (hex, default is off/black):

```bash
blink1-mic-monitor --unmuted-color "#00ff00"
```

### All Options

```
Usage: blink1-mic-monitor [OPTIONS]

Options:
      --list-devices           List available input devices and exit
  -d, --device <DEVICE>        Input device index (default: system default)
  -o, --output <OUTPUT>        Output method [default: blink1] [possible values: console, blink1]
  -v, --verbose                Show live audio level meter
      --muted-color <COLOR>    Blink(1) color when muted [default: #ff0000]
      --unmuted-color <COLOR>  Blink(1) color when unmuted [default: off]
  -h, --help                   Print help
  -V, --version                Print version
```

## Udev Rule

The `install.sh` script automatically creates the required udev rule for non-root USB access. If you need to set it up manually:

```
# /etc/udev/rules.d/51-blink1.rules
SUBSYSTEM=="usb", ATTRS{idVendor}=="27b8", ATTRS{idProduct}=="01ed", MODE:="0666"
```

Then reload udev rules:

```bash
sudo udevadm control --reload-rules
sudo udevadm trigger
```

## Launching

After installation, `blink1-mic-monitor` can be run from your terminal or launched as **Mic Monitor** from your desktop application menu. Right-click the system tray icon and select **Exit** to stop it, or press `Ctrl+C` in the terminal.
