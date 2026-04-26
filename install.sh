#!/bin/bash
set -e

APP_NAME="blink1-mic-monitor"
INSTALL_BIN_DIR="$HOME/.local/bin"
INSTALL_DESKTOP_DIR="$HOME/.local/share/applications"

install_dependencies() {
    echo "Checking for system dependencies..."
    if command -v dnf >/dev/null; then
        echo "Fedora/RHEL detected. installing dependencies..."
        # libayatana-appindicator-gtk3-devel is often the modern replacement, 
        # but libappindicator-gtk3-devel is standard for the rust crate usually.
        sudo dnf install -y alsa-lib-devel libusb1-devel gtk3-devel libappindicator-gtk3-devel
    elif command -v apt-get >/dev/null; then
        echo "Debian/Ubuntu detected. installing dependencies..."
        sudo apt-get install -y libasound2-dev libusb-1.0-0-dev libgtk-3-dev libappindicator3-dev build-essential
    elif command -v pacman >/dev/null; then
        echo "Arch Linux detected. installing dependencies..."
        sudo pacman -S --needed alsa-lib libusb gtk3 libappindicator-gtk3 base-devel
    else
        echo "Warning: Could not detect package manager (dnf/apt/pacman)."
        echo "Please ensure you have development headers for ALSA, libusb, and GTK3 installed."
    fi
}

# Ask to install dependencies
read -p "Do you want to install build dependencies? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    install_dependencies
fi

echo "Building release binary..."
cargo build --release

echo "Installing binary to $INSTALL_BIN_DIR..."
mkdir -p "$INSTALL_BIN_DIR"
cp "target/release/$APP_NAME" "$INSTALL_BIN_DIR/"

echo "Installing desktop entry to $INSTALL_DESKTOP_DIR..."
mkdir -p "$INSTALL_DESKTOP_DIR"
cp "$APP_NAME.desktop" "$INSTALL_DESKTOP_DIR/"

# Update desktop database
update-desktop-database "$INSTALL_DESKTOP_DIR" 2>/dev/null || true

# Udev rules setup
RULES_FILE="/etc/udev/rules.d/51-blink1.rules"
if [ ! -f "$RULES_FILE" ]; then
    echo "Blink(1) udev rules not found."
    echo "Creating $RULES_FILE to allow non-root access... (requires sudo)"
    # Using specific blink(1) vid/pid
    echo 'SUBSYSTEM=="usb", ATTRS{idVendor}=="27b8", ATTRS{idProduct}=="01ed", MODE:="0666"' | sudo tee "$RULES_FILE" > /dev/null
    
    echo "Reloading udev rules..."
    sudo udevadm control --reload-rules
    sudo udevadm trigger
    echo "Udev rules installed."
else
    echo "Blink(1) udev rules detected."
fi

echo "Installation complete!"
echo "Run '$APP_NAME' from your terminal or launch 'Mic Monitor' from your application menu."
