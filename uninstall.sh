#!/bin/bash
set -e

APP_NAME="blink1-mic-monitor"
INSTALL_BIN_DIR="$HOME/.local/bin"
INSTALL_DESKTOP_DIR="$HOME/.local/share/applications"

echo "Uninstalling $APP_NAME..."

if [ -f "$INSTALL_BIN_DIR/$APP_NAME" ]; then
    echo "Removing binary from $INSTALL_BIN_DIR..."
    rm "$INSTALL_BIN_DIR/$APP_NAME"
else
    echo "Binary not found in $INSTALL_BIN_DIR."
fi

if [ -f "$INSTALL_DESKTOP_DIR/$APP_NAME.desktop" ]; then
    echo "Removing desktop entry from $INSTALL_DESKTOP_DIR..."
    rm "$INSTALL_DESKTOP_DIR/$APP_NAME.desktop"
    # Update desktop database
    update-desktop-database "$INSTALL_DESKTOP_DIR" 2>/dev/null || true
else
    echo "Desktop entry not found in $INSTALL_DESKTOP_DIR."
fi

echo "Uninstallation complete!"
