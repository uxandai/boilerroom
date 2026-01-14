#!/usr/bin/env bash
# BoilerRoom + SLSsteam Installer
# One-liner: curl -H 'Accept: application/vnd.github.v3.raw' -fsSL https://api.github.com/repos/uxandai/boilerroom/contents/install.sh?ref=main | bash
# Or clone and run: ./install.sh

set -eu

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Paths
BINARY_NAME="boilerroom"
INSTALL_DIR="$HOME/.local/bin"
DESKTOP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor"

# SLSsteam paths
SLSSTEAM_DIR="$HOME/.local/share/SLSsteam"
SLSSTEAM_CONFIG_DIR="$HOME/.config/SLSsteam"
STEAM_DIR="$HOME/.steam/steam"
FLATPAK_STEAM_DIR="$HOME/.var/app/com.valvesoftware.Steam/.steam/steam"

# GitHub release info
GITHUB_REPO="uxandai/boilerroom"
APPIMAGE_NAME="BoilerRoom_1.0.0_amd64.AppImage"

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

step() { echo -e "\n${BLUE}▶ $1${NC}"; }
success() { echo -e "  ${GREEN}✅ $1${NC}"; }
warn() { echo -e "  ${YELLOW}⚠️  $1${NC}"; }
info() { echo -e "  ${CYAN}ℹ️  $1${NC}"; }
error() { echo -e "  ${RED}❌ $1${NC}"; }

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

# Check if SLSsteam is installed
check_slssteam_installed() {
    [ -f "$SLSSTEAM_DIR/SLSsteam.so" ]
}

# Check if steam.sh is patched
check_steam_patched() {
    [ -f "$STEAM_PATH/steam.sh" ] && grep -q "LD_AUDIT=" "$STEAM_PATH/steam.sh" 2>/dev/null
}

# Check if steam-jupiter is patched
check_jupiter_patched() {
    [ -f "/usr/bin/steam-jupiter" ] && grep -q "LD_AUDIT=" "/usr/bin/steam-jupiter" 2>/dev/null
}

# Check if desktop entry is patched
check_desktop_patched() {
    [ -f "$DESKTOP_DIR/steam.desktop" ] && grep -q "LD_AUDIT=" "$DESKTOP_DIR/steam.desktop" 2>/dev/null
}

# Check if config exists
check_config_exists() {
    [ -f "$SLSSTEAM_CONFIG_DIR/config.yaml" ]
}

# Download and install BoilerRoom AppImage
install_boilerroom() {
    step "Installing BoilerRoom..."
    
    mkdir -p "$INSTALL_DIR"
    mkdir -p "$DESKTOP_DIR"
    mkdir -p "$ICON_DIR/128x128/apps"
    
    # Download latest AppImage from GitHub
    info "Downloading BoilerRoom AppImage from GitHub..."
    local download_url="https://github.com/$GITHUB_REPO/releases/latest/download/$APPIMAGE_NAME"
    
    if ! curl -fL -o "$INSTALL_DIR/$BINARY_NAME" "$download_url" 2>/dev/null; then
        # Fallback to specific version
        download_url="https://github.com/$GITHUB_REPO/releases/download/v1.0.0/$APPIMAGE_NAME"
        if ! curl -fL -o "$INSTALL_DIR/$BINARY_NAME" "$download_url" 2>/dev/null; then
            error "Failed to download BoilerRoom"
            return 1
        fi
    fi
    
    chmod +x "$INSTALL_DIR/$BINARY_NAME"
    success "AppImage installed to $INSTALL_DIR/$BINARY_NAME"
    
    # Extract icon from AppImage (optional, create placeholder if fails)
    "$INSTALL_DIR/$BINARY_NAME" --appimage-extract "*.png" 2>/dev/null || true
    if [ -f "squashfs-root/boilerroom.png" ]; then
        cp "squashfs-root/boilerroom.png" "$ICON_DIR/128x128/apps/$BINARY_NAME.png"
        rm -rf squashfs-root
        success "Icon installed"
    fi
    
    # Create desktop entry
    cat > "$DESKTOP_DIR/$BINARY_NAME.desktop" << EOF
[Desktop Entry]
Name=BoilerRoom
Comment=Steam Deck Game Manager
Exec=$INSTALL_DIR/$BINARY_NAME
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
    command -v update-desktop-database &>/dev/null && update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
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
    
    local temp_dir=$(mktemp -d)
    cd "$temp_dir"
    
    info "Downloading latest SLSsteam from GitHub..."
    local download_url=$(curl -s "https://api.github.com/repos/AceSLS/SLSsteam/releases/latest" \
        | grep "browser_download_url" \
        | grep "SLSsteam-Any.7z" \
        | cut -d '"' -f 4)
    
    if [ -z "$download_url" ]; then
        error "Failed to get SLSsteam download URL"
        cd - >/dev/null
        rm -rf "$temp_dir"
        return 1
    fi
    
    if ! curl -L -o SLSsteam-Any.7z "$download_url" 2>/dev/null; then
        error "Failed to download SLSsteam"
        cd - >/dev/null
        rm -rf "$temp_dir"
        return 1
    fi
    
    # Extract
    if command -v 7z &>/dev/null; then
        7z x SLSsteam-Any.7z -aoa >/dev/null
    elif command -v 7zz &>/dev/null; then
        7zz x SLSsteam-Any.7z -aoa >/dev/null
    else
        error "7z not found. Please install p7zip"
        cd - >/dev/null
        rm -rf "$temp_dir"
        return 1
    fi
    
    # Copy SLSsteam.so
    local found_so=$(find . -name "SLSsteam.so" -type f | head -1)
    if [ -n "$found_so" ]; then
        cp "$found_so" "$SLSSTEAM_DIR/"
        local found_inject=$(find . -name "library-inject.so" -type f | head -1)
        [ -n "$found_inject" ] && cp "$found_inject" "$SLSSTEAM_DIR/"
    else
        error "SLSsteam.so not found in archive"
        cd - >/dev/null
        rm -rf "$temp_dir"
        return 1
    fi
    
    chmod 755 "$SLSSTEAM_DIR"/*.so 2>/dev/null || true
    
    cd - >/dev/null
    rm -rf "$temp_dir"
    
    success "SLSsteam installed to $SLSSTEAM_DIR"
}

# Create SLSsteam config
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

# Patch steam.sh
patch_steam_sh() {
    step "Patching Steam..."
    
    if [ -z "$STEAM_PATH" ] || [ ! -f "$STEAM_PATH/steam.sh" ]; then
        warn "steam.sh not found, skipping..."
        return 0
    fi
    
    if check_steam_patched; then
        success "steam.sh already patched"
        return 0
    fi
    
    local ld_audit_value
    if [ -f "$SLSSTEAM_DIR/library-inject.so" ]; then
        ld_audit_value="$SLSSTEAM_DIR/library-inject.so:$SLSSTEAM_DIR/SLSsteam.so"
    else
        ld_audit_value="$SLSSTEAM_DIR/SLSsteam.so"
    fi
    
    sed -i "2i export LD_AUDIT=\"$ld_audit_value\"" "$STEAM_PATH/steam.sh"
    success "steam.sh patched with LD_AUDIT"
}

# Patch steam-jupiter for Gaming Mode
patch_steam_jupiter() {
    [ ! -f "/usr/bin/steam-jupiter" ] && return 0
    
    step "Patching steam-jupiter (Gaming Mode)..."
    
    if check_jupiter_patched; then
        success "steam-jupiter already patched"
        return 0
    fi
    
    local ld_audit_value
    if [ -f "$SLSSTEAM_DIR/library-inject.so" ]; then
        ld_audit_value="$SLSSTEAM_DIR/library-inject.so:$SLSSTEAM_DIR/SLSsteam.so"
    else
        ld_audit_value="$SLSSTEAM_DIR/SLSsteam.so"
    fi
    
    sudo cp /usr/bin/steam-jupiter "$SLSSTEAM_CONFIG_DIR/steam-jupiter.bak" 2>/dev/null || true
    sudo sed -i "s|^exec /usr/lib/steam/steam|exec env LD_AUDIT=\"$ld_audit_value\" /usr/lib/steam/steam|" /usr/bin/steam-jupiter 2>/dev/null || {
        warn "Could not patch steam-jupiter (might need sudo)"
        return 0
    }
    
    success "steam-jupiter patched for Gaming Mode"
}

# Patch steam.desktop
patch_steam_desktop() {
    step "Patching Steam desktop entry..."
    
    if check_desktop_patched; then
        success "Steam desktop entry already patched"
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
    command -v steamos-readonly &>/dev/null || return 0
    
    step "Steam Deck detected - Running SteamOS setup..."
    
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
    
    if passwd -S deck 2>/dev/null | grep -q "NP"; then
        info "Password not set for 'deck' user"
        if ask "Set password now? (required for sudo operations)" "y"; then
            passwd deck
            success "Password set"
        fi
    fi
}

# Print summary
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
    
    steamdeck_setup
    
    # Install BoilerRoom
    install_boilerroom
    
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
    
    install_slssteam
    create_slssteam_config
    patch_steam_sh
    patch_steam_jupiter
    patch_steam_desktop
    
    print_summary
}

main "$@"
