#!/usr/bin/env python3
"""
Mic Mute Status Monitor

Monitors microphone input levels and indicates mute status via
configurable outputs (console, Blink(1) LED, etc.)

Usage:
    python mic_monitor.py                    # Run with defaults
    python mic_monitor.py --list-devices     # Show available mics
"""

# portaudio-devel

import argparse
import signal
import sys
import time
from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Optional

import numpy as np
import sounddevice as sd


DEFAULT_MUTED_COLOR = "#ff0000"


@dataclass
class Config:
    """Configuration for the mic monitor."""
    mute_value: float = -100.0    # db value when muted.
    sample_rate: int = 16000      # Hz - low is fine for level detection
    block_duration_ms: int = 100  # ms per audio block
    device: Optional[int] = None  # None = default input device


# =============================================================================
# Status Outputs
# =============================================================================

class StatusOutput(ABC):
    """Abstract base for status outputs."""

    @abstractmethod
    def set_muted(self) -> None:
        """Indicate muted state."""
        pass

    @abstractmethod
    def set_unmuted(self) -> None:
        """Indicate unmuted state."""
        pass

    @abstractmethod
    def cleanup(self) -> None:
        """Clean up resources."""
        pass


class ConsoleOutput(StatusOutput):
    """Console-based status output for testing."""

    def __init__(self, show_levels: bool = False):
        self._show_levels = show_levels
        self._current_state: Optional[bool] = None

    def set_muted(self) -> None:
        if self._current_state is not True:
            self._current_state = True
            print("MUTED")

    def set_unmuted(self) -> None:
        if self._current_state is not False:
            self._current_state = False
            print("UNMUTED")

    def cleanup(self) -> None:
        print("\nShutting down...")


class Blink1Output(StatusOutput):
    """
    Blink(1) USB LED status output.
    
    Requires: pip install blink1
    """

    def __init__(self, muted_color: str = DEFAULT_MUTED_COLOR, unmuted_color: Optional[str] = None):
        """
        Initialize Blink(1) output.
        
        Args:
            muted_color: Hex color when muted (default: red)
            unmuted_color: Hex color when unmuted (default: None = off)
        """
        try:
            from blink1.blink1 import Blink1
            self._blink1 = Blink1()
        except ImportError:
            print("Error: blink1 library not installed.", file=sys.stderr)
            print("Install with: pip install blink1", file=sys.stderr)
            sys.exit(1)
        except Exception as e:
            print(f"Error: Could not connect to Blink(1): {e}", file=sys.stderr)
            sys.exit(1)

        self._muted_color = self._parse_color(muted_color)
        self._unmuted_color = self._parse_color(unmuted_color) if unmuted_color else (0, 0, 0)
        self._current_state: Optional[bool] = None

    @staticmethod
    def _parse_color(hex_color: str) -> tuple[int, int, int]:
        """Parse hex color string to RGB tuple."""
        hex_color = hex_color.lstrip('#')
        r = int(hex_color[0:2], 16)
        g = int(hex_color[2:4], 16)
        b = int(hex_color[4:6], 16)
        return (r, g, b)

    def set_muted(self) -> None:
        if self._current_state is not True:
            self._current_state = True
            r, g, b = self._muted_color
            self._blink1.fade_to_rgb(300, r, g, b)

    def set_unmuted(self) -> None:
        if self._current_state is not False:
            self._current_state = False
            r, g, b = self._unmuted_color
            self._blink1.fade_to_rgb(300, r, g, b)

    def cleanup(self) -> None:
        """Turn off LED on shutdown."""
        self._blink1.fade_to_rgb(100, 0, 0, 0)
        self._blink1.close()


# =============================================================================
# Core Monitor
# =============================================================================

class MicMonitor:
    """Monitors microphone input levels and reports mute status."""

    def __init__(self, config: Config, output: StatusOutput, verbose: bool = False):
        self._config = config
        self._output = output
        self._verbose = verbose
        self._running = False
        self._is_muted = True

        # Calculate derived values
        self._block_size = int(config.sample_rate * config.block_duration_ms / 1000)

    def _calculate_db(self, rms: float) -> float:
        """Convert RMS amplitude to dB."""
        if rms > 0:
            return 20 * np.log10(rms)
        return -100.0

    def _audio_callback(
        self,
        indata: np.ndarray,
        frames: int,
        time_info: dict,
        status: sd.CallbackFlags
    ) -> None:
        """Called for each audio block."""
        if status:
            print(f"Audio status: {status}", file=sys.stderr)

        # Calculate RMS amplitude
        rms = np.sqrt(np.mean(indata ** 2))
        db = self._calculate_db(rms)

        if self._verbose:
            bar_len = max(0, int((db + 60) / 2))  # -60dB to 0dB mapped to 0-30
            bar = "#" * bar_len + "-" * (30 - bar_len)
            print(f"\r  Level: [{bar}] {db:6.1f} dB", end="", flush=True)

        if db == self._config.mute_value:
            if not self._is_muted:
                self._is_muted = True
                if self._verbose:
                    print()
                self._output.set_muted()
        else:
            if self._is_muted:
                self._is_muted = False
                if self._verbose:
                    print()
                self._output.set_unmuted()

    def run(self) -> None:
        """Start monitoring. Blocks until stop() is called or interrupted."""
        self._running = True

        # Set initial state to muted
        self._output.set_muted()

        device_name = "default"
        if self._config.device is not None:
            device_info = sd.query_devices(self._config.device)
            device_name = device_info['name']

        try:
            with sd.InputStream(
                device=self._config.device,
                channels=1,
                samplerate=self._config.sample_rate,
                blocksize=self._block_size,
                callback=self._audio_callback
            ):
                print(f"Monitoring: {device_name}")
                print("Press Ctrl+C to stop\n")

                while self._running:
                    time.sleep(0.1)

        except KeyboardInterrupt:
            pass
        finally:
            if self._verbose:
                print()  # Clear the level meter line
            self._output.cleanup()

    def stop(self) -> None:
        """Stop monitoring."""
        self._running = False


# =============================================================================
# CLI
# =============================================================================

def list_devices() -> None:
    """List available audio input devices."""
    print("Available input devices:\n")
    devices = sd.query_devices()
    default_input = sd.default.device[0]

    for i, dev in enumerate(devices):
        if dev['max_input_channels'] > 0:
            marker = "→" if i == default_input else " "
            default_label = " (default)" if i == default_input else ""
            print(f"  {marker} [{i}] {dev['name']}{default_label}")
    print()


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Monitor microphone mute status",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s                         Run with default settings
  %(prog)s --list-devices          Show available microphones
  %(prog)s -d 5                    Use device index 5
  %(prog)s -v                      Show live audio levels
  %(prog)s -o blink1               Output to Blink(1) LED
        """
    )
    parser.add_argument(
        "--list-devices", action="store_true",
        help="List available input devices and exit"
    )
    parser.add_argument(
        "-d", "--device", type=int, default=None,
        help="Input device index (default: system default)"
    )
    parser.add_argument(
        "-o", "--output", choices=["console", "blink1"], default="blink1",
        help="Output method (default: blink1)"
    )
    parser.add_argument(
        "-v", "--verbose", action="store_true",
        help="Show live audio level meter"
    )
    parser.add_argument(
        "--muted-color", default=DEFAULT_MUTED_COLOR,
        help=f"Blink(1) color when muted (default: {DEFAULT_MUTED_COLOR} red)"
    )
    parser.add_argument(
        "--unmuted-color", default=None,
        help="Blink(1) color when unmuted (default: off)"
    )

    args = parser.parse_args()

    if args.list_devices:
        list_devices()
        return

    config = Config(
        device=args.device,
    )

    # Create appropriate output handler
    if args.output == "console":
        output = ConsoleOutput()
    else:
        output = Blink1Output(
            muted_color=args.muted_color,
            unmuted_color=args.unmuted_color
        )

    monitor = MicMonitor(config, output, verbose=args.verbose)

    # Handle signals gracefully
    def signal_handler(sig, frame):
        monitor.stop()

    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)

    monitor.run()


if __name__ == "__main__":
    main()