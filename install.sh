#!/bin/bash
# BoilerRoom - Linux Installation Script
# This script installs BoilerRoom from a compiled release binary
# Run with: ./install.sh

set -e

# Configuration
APP_NAME="boilerroom"
APP_DISPLAY_NAME="BoilerRoom"
BINARY_NAME="boilerroom"
RELEASE_PATH="src-tauri/target/release/$BINARY_NAME"
INSTALL_DIR="$HOME/.local/bin"
ICON_DIR="$HOME/.local/share/icons/hicolor"
DESKTOP_DIR="$HOME/.local/share/applications"

echo "╔═══════════════════════════════════════════════════════╗"
echo "║              BoilerRoom Installer 🎮                  ║"
echo "╚═══════════════════════════════════════════════════════╝"
echo ""

# Check if binary exists
if [ ! -f "$RELEASE_PATH" ]; then
    echo "❌ Error: Binary not found at $RELEASE_PATH"
    echo "   Please build it first with: cd src-tauri && cargo build --release"
    exit 1
fi

# Create directories
echo "📁 Creating directories..."
mkdir -p "$INSTALL_DIR"
mkdir -p "$ICON_DIR/32x32/apps"
mkdir -p "$ICON_DIR/64x64/apps"
mkdir -p "$ICON_DIR/128x128/apps"
mkdir -p "$ICON_DIR/256x256/apps"
mkdir -p "$DESKTOP_DIR"

# Install binary
echo "📦 Installing binary..."
cp "$RELEASE_PATH" "$INSTALL_DIR/$BINARY_NAME"
chmod +x "$INSTALL_DIR/$BINARY_NAME"
echo "   ✅ Installed to $INSTALL_DIR/$BINARY_NAME"

# Install icons
echo "🎨 Installing icons..."
if [ -f "src-tauri/icons/32x32.png" ]; then
    cp src-tauri/icons/32x32.png "$ICON_DIR/32x32/apps/$APP_NAME.png"
fi
if [ -f "src-tauri/icons/64x64.png" ]; then
    cp src-tauri/icons/64x64.png "$ICON_DIR/64x64/apps/$APP_NAME.png"
fi
if [ -f "src-tauri/icons/128x128.png" ]; then
    cp src-tauri/icons/128x128.png "$ICON_DIR/128x128/apps/$APP_NAME.png"
fi
if [ -f "src-tauri/icons/128x128@2x.png" ]; then
    cp src-tauri/icons/128x128@2x.png "$ICON_DIR/256x256/apps/$APP_NAME.png"
fi
echo "   ✅ Icons installed"

# Create desktop entry with WebKit workaround for SteamOS
echo "🖥️  Creating desktop entry..."
cat > "$DESKTOP_DIR/$APP_NAME.desktop" << EOF
[Desktop Entry]
Name=$APP_DISPLAY_NAME
Comment=Steam Deck game manager
Exec=env WEBKIT_DISABLE_COMPOSITING_MODE=1 $INSTALL_DIR/$BINARY_NAME
Icon=$APP_NAME
Terminal=false
Type=Application
Categories=Game;Utility;
Keywords=Steam;Deck;Games;Manager;
StartupWMClass=$APP_DISPLAY_NAME
EOF
chmod +x "$DESKTOP_DIR/$APP_NAME.desktop"
echo "   ✅ Desktop entry created"

# Update desktop database
if command -v update-desktop-database &> /dev/null; then
    update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
fi

# Update icon cache
if command -v gtk-update-icon-cache &> /dev/null; then
    gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || true
fi

# Check if ~/.local/bin is in PATH
if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    echo ""
    echo "⚠️  Note: $HOME/.local/bin is not in your PATH"
    echo "   Add this line to your ~/.bashrc or ~/.zshrc:"
    echo '   export PATH="$HOME/.local/bin:$PATH"'
fi

echo ""
echo "╔═══════════════════════════════════════════════════════╗"
echo "║              🎉 Installation Complete!                ║"
echo "╠═══════════════════════════════════════════════════════╣"
echo "║                                                       ║"
echo "║  Launch BoilerRoom from your application menu         ║"
echo "║  or run: $BINARY_NAME                                 ║"
echo "║                                                       ║"
echo "╚═══════════════════════════════════════════════════════╝"
echo ""
