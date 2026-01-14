#!/usr/bin/env bash
# BoilerRoom + SLSsteam Installer
# Interactive installer for Steam Deck / Linux
# Run: ./install.sh

set -eu

# Colors for pretty output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY_NAME="boilerroom"
COMPILED_DIR="$SCRIPT_DIR/compiled"
INSTALL_DIR="$HOME/.local/bin"
DESKTOP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor"

# SLSsteam paths
SLSSTEAM_DIR="$HOME/.local/share/SLSsteam"
SLSSTEAM_CONFIG_DIR="$HOME/.config/SLSsteam"
STEAM_DIR="$HOME/.steam/steam"
FLATPAK_STEAM_DIR="$HOME/.var/app/com.valvesoftware.Steam/.steam/steam"

# Detect Steam installation type
detect_steam() {
    if [ -d "$FLATPAK_STEAM_DIR" ]; then
        STEAM_TYPE="flatpak"
        STEAM_PATH="$FLATPAK_STEAM_DIR"
        SLSSTEAM_DIR="$HOME/.var/app/com.valvesoftware.Steam/.local/share/SLSsteam"
        SLSSTEAM_CONFIG_DIR="$HOME/.var/app/com.valvesoftware.Steam/.config/SLSsteam"
    elif [ -d "$STEAM_DIR" ]; then
        STEAM_TYPE="native"
        STEAM_PATH="$STEAM_DIR"
    else
        STEAM_TYPE="none"
        STEAM_PATH=""
    fi
}

# Print banner
print_banner() {
    echo -e "${CYAN}"
    echo "╔═══════════════════════════════════════════════════════╗"
    echo "║          BoilerRoom + SLSsteam Installer 🎮           ║"
    echo "║                                                       ║"
    echo "║  Steam Deck / Linux Game Manager                      ║"
    echo "╚═══════════════════════════════════════════════════════╝"
    echo -e "${NC}"
}

# Print step
step() {
    echo -e "\n${BLUE}▶ $1${NC}"
}

# Print success
success() {
    echo -e "  ${GREEN}✅ $1${NC}"
}

# Print warning
warn() {
    echo -e "  ${YELLOW}⚠️  $1${NC}"
}

# Print info
info() {
    echo -e "  ${CYAN}ℹ️  $1${NC}"
}

# Print error
error() {
    echo -e "  ${RED}❌ $1${NC}"
}

# Ask yes/no question
ask() {
    local prompt="$1"
    local default="${2:-n}"
    local response
    
    if [ "$default" = "y" ]; then
        read -p "  $prompt [Y/n]: " response
        response=${response:-y}
    else
        read -p "  $prompt [y/N]: " response
        response=${response:-n}
    fi
    
    [[ "$response" =~ ^[Yy]$ ]]
}

# Check if SLSsteam is already installed and configured
check_slssteam_installed() {
    if [ -f "$SLSSTEAM_DIR/SLSsteam.so" ]; then
        return 0
    fi
    return 1
}

# Check if steam.sh is already patched
check_steam_patched() {
    if [ -f "$STEAM_PATH/steam.sh" ]; then
        if grep -q "LD_AUDIT=" "$STEAM_PATH/steam.sh" 2>/dev/null; then
            return 0
        fi
    fi
    return 1
}

# Check if steam-jupiter is patched (Steam Deck Gaming Mode)
check_jupiter_patched() {
    if [ -f "/usr/bin/steam-jupiter" ]; then
        if grep -q "LD_AUDIT=" "/usr/bin/steam-jupiter" 2>/dev/null; then
            return 0
        fi
    fi
    return 1
}

# Check if desktop entry is configured with LD_AUDIT
check_desktop_patched() {
    local desktop_file="$DESKTOP_DIR/steam.desktop"
    if [ -f "$desktop_file" ]; then
        if grep -q "LD_AUDIT=" "$desktop_file" 2>/dev/null; then
            return 0
        fi
    fi
    return 1
}

# Check if config.yaml exists
check_config_exists() {
    if [ -f "$SLSSTEAM_CONFIG_DIR/config.yaml" ]; then
        return 0
    fi
    return 1
}

# Install BoilerRoom binary
install_boilerroom() {
    step "Installing BoilerRoom..."
    
    local binary_path="$COMPILED_DIR/$BINARY_NAME"
    
    if [ ! -f "$binary_path" ]; then
        error "Binary not found at $binary_path"
        info "Please compile first or place the binary in compiled/"
        return 1
    fi
    
    mkdir -p "$INSTALL_DIR"
    mkdir -p "$ICON_DIR/32x32/apps"
    mkdir -p "$ICON_DIR/64x64/apps"
    mkdir -p "$ICON_DIR/128x128/apps"
    mkdir -p "$DESKTOP_DIR"
    
    # Copy binary
    cp "$binary_path" "$INSTALL_DIR/$BINARY_NAME"
    chmod +x "$INSTALL_DIR/$BINARY_NAME"
    success "Binary installed to $INSTALL_DIR/$BINARY_NAME"
    
    # Copy icons if available
    if [ -d "$SCRIPT_DIR/source/src-tauri/icons" ]; then
        [ -f "$SCRIPT_DIR/source/src-tauri/icons/32x32.png" ] && cp "$SCRIPT_DIR/source/src-tauri/icons/32x32.png" "$ICON_DIR/32x32/apps/$BINARY_NAME.png"
        [ -f "$SCRIPT_DIR/source/src-tauri/icons/64x64.png" ] && cp "$SCRIPT_DIR/source/src-tauri/icons/64x64.png" "$ICON_DIR/64x64/apps/$BINARY_NAME.png"
        [ -f "$SCRIPT_DIR/source/src-tauri/icons/128x128.png" ] && cp "$SCRIPT_DIR/source/src-tauri/icons/128x128.png" "$ICON_DIR/128x128/apps/$BINARY_NAME.png"
        success "Icons installed"
    fi
    
    # Create desktop entry with WebKit workaround
    cat > "$DESKTOP_DIR/$BINARY_NAME.desktop" << EOF
[Desktop Entry]
Name=BoilerRoom
Comment=Steam Deck Game Manager
Exec=env WEBKIT_DISABLE_COMPOSITING_MODE=1 $INSTALL_DIR/$BINARY_NAME
Icon=$BINARY_NAME
Terminal=false
Type=Application
Categories=Game;Utility;
Keywords=Steam;Deck;Games;Manager;
StartupWMClass=BoilerRoom
EOF
    chmod +x "$DESKTOP_DIR/$BINARY_NAME.desktop"
    success "Desktop entry created"
    
    # Update desktop database
    if command -v update-desktop-database &> /dev/null; then
        update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
    fi
}

# Download and install SLSsteam
install_slssteam() {
    step "Installing SLSsteam..."
    
    if check_slssteam_installed; then
        success "SLSsteam already installed at $SLSSTEAM_DIR"
        info "Skipping download to preserve existing installation"
        return 0
    fi
    
    mkdir -p "$SLSSTEAM_DIR"
    mkdir -p "$SLSSTEAM_CONFIG_DIR"
    
    # Download latest SLSsteam
    local temp_dir=$(mktemp -d)
    cd "$temp_dir"
    
    info "Downloading latest SLSsteam from GitHub..."
    local download_url=$(curl -s "https://api.github.com/repos/AceSLS/SLSsteam/releases/latest" \
        | grep "browser_download_url" \
        | grep "SLSsteam-Any.7z" \
        | cut -d '"' -f 4)
    
    if [ -z "$download_url" ]; then
        error "Failed to get SLSsteam download URL"
        cd "$SCRIPT_DIR"
        rm -rf "$temp_dir"
        return 1
    fi
    
    if ! curl -L -o SLSsteam-Any.7z "$download_url" 2>/dev/null; then
        error "Failed to download SLSsteam"
        cd "$SCRIPT_DIR"
        rm -rf "$temp_dir"
        return 1
    fi
    
    # Extract
    if command -v 7z &> /dev/null; then
        7z x SLSsteam-Any.7z -aoa >/dev/null
    elif command -v 7zz &> /dev/null; then
        7zz x SLSsteam-Any.7z -aoa >/dev/null
    elif [ -f "$SCRIPT_DIR/7zz" ]; then
        chmod +x "$SCRIPT_DIR/7zz"
        "$SCRIPT_DIR/7zz" x SLSsteam-Any.7z -aoa >/dev/null
    else
        error "7z not found. Please install p7zip-full"
        cd "$SCRIPT_DIR"
        rm -rf "$temp_dir"
        return 1
    fi
    
    # Copy SLSsteam.so
    if [ -f "bin/SLSsteam.so" ]; then
        cp "bin/SLSsteam.so" "$SLSSTEAM_DIR/"
        [ -f "bin/library-inject.so" ] && cp "bin/library-inject.so" "$SLSSTEAM_DIR/"
    elif [ -f "SLSsteam.so" ]; then
        cp "SLSsteam.so" "$SLSSTEAM_DIR/"
        [ -f "library-inject.so" ] && cp "library-inject.so" "$SLSSTEAM_DIR/"
    else
        local found_so=$(find . -name "SLSsteam.so" -type f | head -1)
        if [ -n "$found_so" ]; then
            cp "$found_so" "$SLSSTEAM_DIR/"
            local found_inject=$(find . -name "library-inject.so" -type f | head -1)
            [ -n "$found_inject" ] && cp "$found_inject" "$SLSSTEAM_DIR/"
        else
            error "SLSsteam.so not found in archive"
            cd "$SCRIPT_DIR"
            rm -rf "$temp_dir"
            return 1
        fi
    fi
    
    chmod 755 "$SLSSTEAM_DIR"/*.so 2>/dev/null || true
    
    # Cleanup
    cd "$SCRIPT_DIR"
    rm -rf "$temp_dir"
    
    success "SLSsteam installed to $SLSSTEAM_DIR"
}

# Create SLSsteam config.yaml
create_slssteam_config() {
    step "Configuring SLSsteam..."
    
    if check_config_exists; then
        success "config.yaml already exists"
        info "Skipping to preserve your existing configuration"
        return 0
    fi
    
    mkdir -p "$SLSSTEAM_CONFIG_DIR"
    
    cat > "$SLSSTEAM_CONFIG_DIR/config.yaml" << 'EOF'
# SLSsteam Configuration
# Created by BoilerRoom installer

DisableFamilyShareLock: yes
UseWhitelist: no
AutoFilterList: yes
AppIds:

PlayNotOwnedGames: yes

AdditionalApps:
  - 480  # Spacewar (test game)

DlcData:
FakeOffline:
FakeAppIds:

IdleStatus:
  AppId: 0
  Title: ""

UnownedStatus:
  AppId: 0
  Title: ""

DenuvoGames:
SafeMode: yes
Notifications: yes
WarnHashMissmatch: no
NotifyInit: yes
API: yes
LogLevel: 2
ExtendedLogging: no
EOF

    success "config.yaml created at $SLSSTEAM_CONFIG_DIR"
}

# Patch steam.sh with LD_AUDIT
patch_steam_sh() {
    step "Patching Steam..."
    
    if [ -z "$STEAM_PATH" ] || [ ! -f "$STEAM_PATH/steam.sh" ]; then
        warn "steam.sh not found, skipping..."
        return 0
    fi
    
    if check_steam_patched; then
        success "steam.sh already patched"
        info "Skipping to preserve existing patch"
        return 0
    fi
    
    # Determine LD_AUDIT path based on install location
    local ld_audit_value
    if [ -f "$SLSSTEAM_DIR/library-inject.so" ]; then
        ld_audit_value="$SLSSTEAM_DIR/library-inject.so:$SLSSTEAM_DIR/SLSsteam.so"
    else
        ld_audit_value="$SLSSTEAM_DIR/SLSsteam.so"
    fi
    
    # Insert LD_AUDIT export after shebang
    sed -i "2i export LD_AUDIT=\"$ld_audit_value\"" "$STEAM_PATH/steam.sh"
    
    success "steam.sh patched with LD_AUDIT"
}

# Patch steam-jupiter for Gaming Mode (Steam Deck)
patch_steam_jupiter() {
    if [ ! -f "/usr/bin/steam-jupiter" ]; then
        return 0
    fi
    
    step "Patching steam-jupiter (Gaming Mode)..."
    
    if check_jupiter_patched; then
        success "steam-jupiter already patched"
        info "Skipping to preserve existing patch"
        return 0
    fi
    
    local ld_audit_value
    if [ -f "$SLSSTEAM_DIR/library-inject.so" ]; then
        ld_audit_value="$SLSSTEAM_DIR/library-inject.so:$SLSSTEAM_DIR/SLSsteam.so"
    else
        ld_audit_value="$SLSSTEAM_DIR/SLSsteam.so"
    fi
    
    # Backup first
    sudo cp /usr/bin/steam-jupiter "$SLSSTEAM_CONFIG_DIR/steam-jupiter.bak" 2>/dev/null || true
    
    # Patch exec line
    sudo sed -i "s|^exec /usr/lib/steam/steam|exec env LD_AUDIT=\"$ld_audit_value\" /usr/lib/steam/steam|" /usr/bin/steam-jupiter 2>/dev/null || {
        warn "Could not patch steam-jupiter (might need sudo)"
        return 0
    }
    
    success "steam-jupiter patched for Gaming Mode"
}

# Create patched steam.desktop
patch_steam_desktop() {
    step "Patching Steam desktop entry..."
    
    if check_desktop_patched; then
        success "Steam desktop entry already patched"
        info "Skipping to preserve existing patch"
        return 0
    fi
    
    local ld_audit_value
    if [ -f "$SLSSTEAM_DIR/library-inject.so" ]; then
        ld_audit_value="$SLSSTEAM_DIR/library-inject.so:$SLSSTEAM_DIR/SLSsteam.so"
    else
        ld_audit_value="$SLSSTEAM_DIR/SLSsteam.so"
    fi
    
    if [ -f "/usr/share/applications/steam.desktop" ]; then
        cp /usr/share/applications/steam.desktop "$DESKTOP_DIR/steam.desktop"
        sed -i "s|^Exec=/|Exec=env LD_AUDIT=\"$ld_audit_value\" /|" "$DESKTOP_DIR/steam.desktop"
        success "Steam desktop entry patched"
    else
        warn "steam.desktop not found"
    fi
}

# Steam Deck specific setup
steamdeck_setup() {
    # Check if SteamOS
    if ! command -v steamos-readonly &> /dev/null; then
        return 0
    fi
    
    step "Steam Deck detected - Running SteamOS setup..."
    
    # Check read-only mode
    local readonly_status=$(steamos-readonly status 2>/dev/null || echo "unknown")
    if echo "$readonly_status" | grep -qi "enabled"; then
        if ask "Disable read-only mode? (required for full setup)"; then
            sudo steamos-readonly disable
            success "Read-only mode disabled"
        else
            warn "Some features may not work without disabling read-only"
        fi
    else
        success "Read-only mode already disabled"
    fi
    
    # Check password
    if passwd -S deck 2>/dev/null | grep -q "NP"; then
        info "Password not set for 'deck' user"
        if ask "Set password now? (required for sudo operations)" "y"; then
            passwd deck
            success "Password set"
        fi
    fi
}

# Enable SSH
setup_ssh() {
    if ! ask "Enable SSH for remote access?"; then
        return 0
    fi
    
    step "Configuring SSH..."
    
    if systemctl is-active --quiet sshd; then
        success "SSH already running"
    else
        sudo systemctl enable sshd
        sudo systemctl start sshd
        success "SSH enabled and started"
    fi
    
    # Show IP
    local ip_addr=$(ip -4 addr show | grep -oP '(?<=inet\s)192\.168\.\d+\.\d+' | head -1 || echo "")
    [ -z "$ip_addr" ] && ip_addr=$(ip -4 addr show | grep -oP '(?<=inet\s)10\.\d+\.\d+\.\d+' | head -1 || echo "")
    [ -z "$ip_addr" ] && ip_addr=$(hostname -I | awk '{print $1}')
    
    if [ -n "$ip_addr" ]; then
        echo ""
        info "Your IP address: ${CYAN}$ip_addr${NC}"
        info "Use this to connect from another PC"
    fi
}

# Print final summary
print_summary() {
    echo ""
    echo -e "${GREEN}"
    echo "╔═══════════════════════════════════════════════════════╗"
    echo "║              🎉 Installation Complete!                ║"
    echo "╠═══════════════════════════════════════════════════════╣"
    echo "║                                                       ║"
    echo "║  BoilerRoom: ~/.local/bin/boilerroom                  ║"
    echo "║  SLSsteam:   ~/.local/share/SLSsteam                  ║"
    echo "║  Config:     ~/.config/SLSsteam/config.yaml           ║"
    echo "║                                                       ║"
    echo "║  Launch from application menu or run: boilerroom      ║"
    echo "║                                                       ║"
    echo "║  ⚠️  RESTART STEAM for SLSsteam to take effect!       ║"
    echo "║                                                       ║"
    echo "╚═══════════════════════════════════════════════════════╝"
    echo -e "${NC}"
    
    # Check if ~/.local/bin is in PATH
    if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
        echo ""
        warn "$HOME/.local/bin is not in your PATH"
        info "Add to your ~/.bashrc or ~/.zshrc:"
        echo '    export PATH="$HOME/.local/bin:$PATH"'
    fi
}

# Main
main() {
    print_banner
    
    detect_steam
    
    if [ "$STEAM_TYPE" = "none" ]; then
        error "Steam not detected!"
        info "Please install Steam first"
        exit 1
    fi
    
    info "Detected: $STEAM_TYPE Steam installation"
    echo ""
    
    # Steam Deck specific
    steamdeck_setup
    
    # Install BoilerRoom
    if [ -f "$COMPILED_DIR/$BINARY_NAME" ]; then
        install_boilerroom
    else
        warn "BoilerRoom binary not found in compiled/"
        info "Skipping BoilerRoom installation"
    fi
    
    # Ask about SLSsteam
    echo ""
    if check_slssteam_installed && check_config_exists && check_steam_patched; then
        success "SLSsteam appears to be fully configured"
        if ! ask "Reinstall/reconfigure SLSsteam anyway?"; then
            print_summary
            return 0
        fi
    else
        if ! ask "Install/configure SLSsteam? (required for playing unowned games)" "y"; then
            print_summary
            return 0
        fi
    fi
    
    # Install SLSsteam
    install_slssteam
    create_slssteam_config
    patch_steam_sh
    patch_steam_jupiter
    patch_steam_desktop
    
    # SSH setup
    setup_ssh
    
    print_summary
}

main "$@"
