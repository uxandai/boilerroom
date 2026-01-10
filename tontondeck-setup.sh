#!/bin/bash
# TonTonDeck - Setup Script for Steam Deck / Linux
# Run this script in Desktop Mode

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APPIMAGE_NAME="TonTonDeck_1.0.0_amd64.AppImage"
INSTALL_DIR="$HOME/.local/bin"
DESKTOP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons"
SLSSTEAM_DIR="$HOME/.local/share/SLSsteam"
SLSSTEAM_CONFIG_DIR="$HOME/.config/SLSsteam"

echo "╔═══════════════════════════════════════════════════════╗"
echo "║              TonTonDeck Setup 🎮                      ║"
echo "╚═══════════════════════════════════════════════════════╝"
echo ""

# Ask about SSH for remote access
echo "Do you want to enable SSH for remote access to this machine?"
echo "(Required if you want to transfer files from another PC)"
echo ""
read -p "Enable SSH? (y/n): " ENABLE_SSH

if [[ "$ENABLE_SSH" =~ ^[YyTt]$ ]]; then
    SSH_ENABLED=true
    echo ""
    echo "✓ Will configure SSH for remote access"
else
    SSH_ENABLED=false
    echo ""
    echo "✓ Skipping SSH configuration"
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Step 1: Set password if not set (needed for sudo)
echo ""
echo "🔐 Checking user password..."
if passwd -S deck 2>/dev/null | grep -q "NP"; then
    echo "   Password not set. Required for sudo operations."
    echo "   Please set a password for user 'deck':"
    passwd deck
    echo "   ✅ Password set"
else
    echo "   ✅ Password already configured"
fi

# Step 2: Disable read-only mode
echo ""
echo "📝 Checking read-only mode..."
if command -v steamos-readonly &> /dev/null; then
    READONLY_STATUS=$(steamos-readonly status 2>/dev/null || echo "unknown")
    if echo "$READONLY_STATUS" | grep -qi "enabled"; then
        echo "   Disabling read-only mode..."
        sudo steamos-readonly disable
        echo "   ✅ Read-only mode disabled"
    else
        echo "   ✅ Read-only mode already disabled"
    fi
else
    echo "   ℹ️  SteamOS not detected, skipping..."
fi

# Step 3: Enable SSH if requested
if [ "$SSH_ENABLED" = true ]; then
    echo ""
    echo "🌐 Configuring SSH..."
    if systemctl is-active --quiet sshd; then
        echo "   ✅ SSH already running"
    else
        echo "   Enabling SSH service..."
        sudo systemctl enable sshd
        sudo systemctl start sshd
        echo "   ✅ SSH enabled and started"
    fi
fi

# Step 4: Clean WebKit cache (prevents white screen issues)
echo ""
echo "🗑️  Cleaning WebKit cache..."
rm -rf "$HOME/.cache/TonTonDeck" 2>/dev/null
rm -rf "$HOME/.cache/com.tontondeck.app" 2>/dev/null
echo "   ✅ Cache cleared"

# Step 5: Install AppImage
echo ""
echo "📦 Installing TonTonDeck..."

# Create directories if needed
mkdir -p "$INSTALL_DIR"
mkdir -p "$DESKTOP_DIR"
mkdir -p "$ICON_DIR"

# Copy AppImage (remove old one first for clean upgrade)
if [ -f "$SCRIPT_DIR/$APPIMAGE_NAME" ]; then
    # Remove existing AppImage if present
    if [ -f "$INSTALL_DIR/$APPIMAGE_NAME" ]; then
        rm -f "$INSTALL_DIR/$APPIMAGE_NAME"
        echo "   🗑️  Removed previous version"
    fi
    cp "$SCRIPT_DIR/$APPIMAGE_NAME" "$INSTALL_DIR/$APPIMAGE_NAME"
    chmod +x "$INSTALL_DIR/$APPIMAGE_NAME"
    echo "   ✅ Copied to $INSTALL_DIR/"
else
    echo "   ⚠️  $APPIMAGE_NAME not found in current directory"
    echo "   Place this script next to the AppImage file"
    exit 1
fi

# Step 6: Extract icon from AppImage
echo ""
echo "🎨 Extracting application icon..."
ICON_PATH="$ICON_DIR/tontondeck.png"

# Try to extract icon from AppImage
cd "$INSTALL_DIR"
if "./$APPIMAGE_NAME" --appimage-extract "*.png" >/dev/null 2>&1; then
    # Find the largest icon
    EXTRACTED_ICON=$(find squashfs-root -name "*.png" -type f 2>/dev/null | head -1)
    if [ -n "$EXTRACTED_ICON" ]; then
        cp "$EXTRACTED_ICON" "$ICON_PATH"
        echo "   ✅ Icon extracted"
    fi
    rm -rf squashfs-root
fi
cd "$SCRIPT_DIR"

# Fallback if extraction failed
if [ ! -f "$ICON_PATH" ]; then
    echo "   ℹ️  Using fallback icon"
    ICON_PATH="steam"
fi

# Step 7: Create desktop entry for TonTonDeck
echo ""
echo "🖥️  Creating menu shortcut..."

cat > "$DESKTOP_DIR/tontondeck.desktop" << EOF
[Desktop Entry]
Name=TonTonDeck
Comment=Install games on Steam Deck
Exec=$INSTALL_DIR/$APPIMAGE_NAME
Icon=$ICON_PATH
Type=Application
Categories=Game;Utility;
Terminal=false
StartupWMClass=TonTonDeck
EOF

chmod +x "$DESKTOP_DIR/tontondeck.desktop"
echo "   ✅ Shortcut created in application menu"

# Update desktop database
update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true

# Step 7: Ask about SLSsteam installation
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "🎮 SLSsteam allows running non-Steam games directly"
echo "   in the Steam interface (Gaming Mode)."
echo ""
read -p "Install SLSsteam? (y/n): " INSTALL_SLSSTEAM

SLSSTEAM_INSTALLED=false

if [[ "$INSTALL_SLSSTEAM" =~ ^[TtYy]$ ]]; then
    echo ""
    echo "⬇️  Installing SLSsteam..."
    
    # Create directories
    mkdir -p "$SLSSTEAM_DIR"
    mkdir -p "$SLSSTEAM_CONFIG_DIR"
    
    # Download SLSsteam from GitHub
    echo "   Downloading SLSsteam from GitHub..."
    TEMP_DIR=$(mktemp -d)
    cd "$TEMP_DIR"
    
    SLSSTEAM_URL="https://github.com/AceSLS/SLSsteam/releases/latest/download/SLSsteam-Any.7z"
    if curl -L -o SLSsteam-Any.7z "$SLSSTEAM_URL" 2>/dev/null; then
        echo "   ✅ Downloaded SLSsteam-Any.7z"
    else
        echo "   ❌ Failed to download SLSsteam"
        cd "$SCRIPT_DIR"
        rm -rf "$TEMP_DIR"
        exit 1
    fi
    
    # Extract using bundled 7zz
    echo "   Extracting archive..."
    chmod +x "$SCRIPT_DIR/7zz"
    
    if ! "$SCRIPT_DIR/7zz" x -y SLSsteam-Any.7z >/dev/null; then
        echo "   ⚠️  Failed to extract archive"
        cd "$SCRIPT_DIR"
        rm -rf "$TEMP_DIR"
        exit 1
    fi
    echo "   ✅ Archive extracted"
    
    # Find and copy SLSsteam.so
    if [ -f "bin/SLSsteam.so" ]; then
        cp "bin/SLSsteam.so" "$SLSSTEAM_DIR/SLSsteam.so"
        chmod 755 "$SLSSTEAM_DIR/SLSsteam.so"
        echo "   ✅ SLSsteam.so installed"
    elif [ -f "SLSsteam.so" ]; then
        cp "SLSsteam.so" "$SLSSTEAM_DIR/SLSsteam.so"
        chmod 755 "$SLSSTEAM_DIR/SLSsteam.so"
        echo "   ✅ SLSsteam.so installed"
    else
        # Try to find it anywhere in extracted files
        FOUND_SO=$(find . -name "SLSsteam.so" -type f | head -1)
        if [ -n "$FOUND_SO" ]; then
            cp "$FOUND_SO" "$SLSSTEAM_DIR/SLSsteam.so"
            chmod 755 "$SLSSTEAM_DIR/SLSsteam.so"
            echo "   ✅ SLSsteam.so installed"
        else
            echo "   ❌ SLSsteam.so not found in archive"
            cd "$SCRIPT_DIR"
            rm -rf "$TEMP_DIR"
            exit 1
        fi
    fi
    
    # Cleanup
    cd "$SCRIPT_DIR"
    rm -rf "$TEMP_DIR"
    
    # Create default config if not exists
    if [ ! -f "$SLSSTEAM_CONFIG_DIR/config.yaml" ]; then
        echo "   Creating default configuration..."
        cat > "$SLSSTEAM_CONFIG_DIR/config.yaml" << 'EOF'
#Example AppIds Config for those not familiar with YAML:
#AppIds:
#  - 440
#  - 730
#Take care of not messing up your spaces! Otherwise it won't work

#Example of DlcData:
#DlcData:
#  AppId:
#    FirstDlcAppId: "Dlc Name"
#    SecondDlcAppId: "Dlc Name"

#Example of DenuvoGames:
#DenuvoGames:
#  SteamId:
#    -  AppId1
#    -  AppId2

#Example of FakeAppIds:
#FakeAppIds:
#  AppId1: FakeAppId1
#  AppId2: FakeAppId2

#Disables Family Share license locking for self and others
DisableFamilyShareLock: yes

#Switches to whitelist instead of the default blacklist
UseWhitelist: no

#Automatically filter Apps in CheckAppOwnership. Filters everything but Games and Applications. Should not affect DLC checks
#Overrides black-/whitelist. Gets overriden by AdditionalApps
AutoFilterList: yes

#List of AppIds to ex-/include
AppIds:

#Enables playing of not owned games. Respects black-/whitelist AppIds
PlayNotOwnedGames: yes

#Additional AppIds to inject (Overrides your black-/whitelist & also overrides OwnerIds for apps you got shared!) Best to use this only on games NOT in your library.
AdditionalApps:
# Game Name
- 480
#Extra Data for Dlcs belonging to a specific AppId. Only needed
#when the App you're playing is hit by Steams 64 DLC limit
DlcData:

#Fake Steam being offline for specified AppIds. Same format as AppIds
FakeOffline:

#Change AppIds of games to enable networking features
#Use 0 as a key to set for all unowned Apps
FakeAppIds:

#Custom ingame statuses. Set AppId to 0 to disable
IdleStatus:
  AppId: 0
  Title: ""

UnownedStatus:
  AppId: 0
  Title: ""

#Blocks games from unlocking on wrong accounts
DenuvoGames:

#Automatically disable SLSsteam when steamclient.so does not match a predefined file hash that is known to work
#You should enable this if you're planing to use SLSsteam with Steam Deck's gamemode
SafeMode: yes

#Toggles notifications via notify-send
Notifications: yes

#Warn user via notification when steamclient.so hash differs from known safe hash
#Mostly useful for development so I don't accidentally miss an update
WarnHashMissmatch: no

#Notify when SLSsteam is done initializing
NotifyInit: yes

#Enable sending commands to SLSsteam via /tmp/SLSsteam.API
API: yes

#Log levels:
#Once = 0
#Debug = 1
#Info = 2
#NotifyShort = 3
#NotifyLong = 4
#Warn = 5
#None = 6
LogLevel: 2

#Logs all calls to Steamworks (this makes the logfile huge! Only useful for debugging/analyzing
ExtendedLogging: no
EOF
        echo "   ✅ config.yaml created"
    else
        echo "   ✅ config.yaml already exists"
    fi
    
    # Patch steam.desktop
    echo "   Modifying steam.desktop..."
    if [ -f /usr/share/applications/steam.desktop ]; then
        cp /usr/share/applications/steam.desktop "$DESKTOP_DIR/steam.desktop"
        sed -i "s|^Exec=/|Exec=env LD_AUDIT=\"$SLSSTEAM_DIR/SLSsteam.so\" /|" "$DESKTOP_DIR/steam.desktop"
        echo "   ✅ steam.desktop modified"
    else
        echo "   ⚠️ steam.desktop not found"
    fi
    
    # Patch steam-jupiter (requires sudo)
    echo "   Modifying steam-jupiter (requires sudo)..."
    if [ -f /usr/bin/steam-jupiter ]; then
        # Backup first
        sudo cp /usr/bin/steam-jupiter "$SLSSTEAM_CONFIG_DIR/steam-jupiter.bak" 2>/dev/null || true
        
        # Check if already patched
        if grep -q "LD_AUDIT" /usr/bin/steam-jupiter 2>/dev/null; then
            echo "   ✅ steam-jupiter already modified"
        else
            # Replace exec line with exec env LD_AUDIT=...
            sudo sed -i 's|^exec /usr/lib/steam/steam|exec env LD_AUDIT="/home/deck/.local/share/SLSsteam/SLSsteam.so" /usr/lib/steam/steam|' /usr/bin/steam-jupiter 2>/dev/null || true
            echo "   ✅ steam-jupiter modified"
        fi
    else
        echo "   ℹ️ steam-jupiter doesn't exist (normal outside Gaming Mode)"
    fi
    
    SLSSTEAM_INSTALLED=true
    echo ""
    echo "   🎉 SLSsteam installed!"
    echo "   ⚠️ Restart Steam for changes to take effect"
else
    echo ""
    echo "   ℹ️ Skipped SLSsteam installation"
    echo "   You can install it later through TonTonDeck"
fi

# Get IP address if SSH was enabled
IP_ADDR=""
if [ "$SSH_ENABLED" = true ]; then
    IP_ADDR=$(ip -4 addr show | grep -oP '(?<=inet\s)192\.168\.\d+\.\d+' | head -1 || echo "")
    if [ -z "$IP_ADDR" ]; then
        IP_ADDR=$(ip -4 addr show | grep -oP '(?<=inet\s)10\.\d+\.\d+\.\d+' | head -1 || echo "")
    fi
    if [ -z "$IP_ADDR" ]; then
        IP_ADDR=$(hostname -I | awk '{print $1}')
    fi
fi

# Final summary
echo ""
echo "╔═══════════════════════════════════════════════════════╗"
echo "║              🎉 Installation Complete!                ║"
echo "╠═══════════════════════════════════════════════════════╣"
echo "║                                                       ║"
echo "║  1. Launch TonTonDeck from Start menu                 ║"
echo "║     (Category: All, Games, or Utilities)              ║"
echo "║                                                       ║"
echo "║  2. You can also add TonTonDeck in Gaming Mode        ║"
echo "║     Steam → Add Game → Add Non-Steam Game             ║"
echo "║                                                       ║"
if [ "$SSH_ENABLED" = true ] && [ -n "$IP_ADDR" ]; then
echo "║  3. Your IP address for remote connection:            ║"
echo "║     📡 $IP_ADDR                                       ║"
echo "║     (Use this in TonTonDeck on your PC)               ║"
echo "║                                                       ║"
fi
if [ "$SLSSTEAM_INSTALLED" = true ]; then
echo "║  ⚠️  Restart Steam for SLSsteam to work!              ║"
echo "║                                                       ║"
fi
echo "╚═══════════════════════════════════════════════════════╝"
echo ""
