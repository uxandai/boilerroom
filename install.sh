#!/usr/bin/env bash
# BoilerRoom Installer
# One-liner: curl -fsSL https://raw.githubusercontent.com/uxandai/boilerroom/main/install.sh | bash

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

# GitHub release info
GITHUB_REPO="uxandai/boilerroom"
APPIMAGE_NAME="BoilerRoom_1.0.0_amd64.AppImage"

# Print banner
print_banner() {
    echo -e "${CYAN}"
    echo "╔═══════════════════════════════════════════════════════╗"
    echo "║               BoilerRoom Installer 🎮                 ║"
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

# Download and install BoilerRoom AppImage
install_boilerroom() {
    step "Installing BoilerRoom..."
    
    mkdir -p "$INSTALL_DIR"
    mkdir -p "$DESKTOP_DIR"
    mkdir -p "$ICON_DIR/128x128/apps"
    
    # Get latest release version from GitHub API (includes prereleases)
    info "Checking latest version..."
    local release_info
    # Use /releases (not /releases/latest) to include prereleases
    release_info=$(curl -fsSL "https://api.github.com/repos/$GITHUB_REPO/releases" 2>/dev/null | head -c 5000 || echo "")
    
    local version="v1.4.0"  # Fallback version
    local download_url=""
    
    if [ -n "$release_info" ]; then
        # Get first (most recent) release tag
        version=$(echo "$release_info" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')
        download_url=$(echo "$release_info" | grep '"browser_download_url"' | grep -i "appimage" | head -1 | sed 's/.*"browser_download_url": *"\([^"]*\)".*/\1/')
    fi
    
    # Fallback URL if API parsing failed
    if [ -z "$download_url" ]; then
        download_url="https://github.com/$GITHUB_REPO/releases/download/$version/$APPIMAGE_NAME"
    fi
    
    info "Downloading BoilerRoom ($version) AppImage from GitHub..."
    
    if ! curl -fL -o "$INSTALL_DIR/$BINARY_NAME" "$download_url" 2>/dev/null; then
        error "Failed to download BoilerRoom"
        return 1
    fi
    
    chmod +x "$INSTALL_DIR/$BINARY_NAME"
    success "AppImage ($version) installed to $INSTALL_DIR/$BINARY_NAME"
    
    # Extract icon from AppImage (optional)
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

# Print summary
print_summary() {
    echo ""
    echo -e "${GREEN}"
    echo "╔═══════════════════════════════════════════════════════╗"
    echo "║              🎉 BoilerRoom Installed!                 ║"
    echo "╠═══════════════════════════════════════════════════════╣"
    echo "║                                                       ║"
    echo "║  Location: ~/.local/bin/boilerroom                    ║"
    echo "║                                                       ║"
    echo "║  Launch from application menu or run: boilerroom      ║"
    echo "║                                                       ║"
    echo "╚═══════════════════════════════════════════════════════╝"
    echo -e "${NC}"
    
    if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
        echo ""
        warn "$HOME/.local/bin is not in your PATH"
        info "Add to your ~/.bashrc or ~/.zshrc:"
        echo '    export PATH="$HOME/.local/bin:$PATH"'
    fi
    
    echo ""
    echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${YELLOW}  📦 SLSsteam Installation (Required for full features)${NC}"
    echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo -e "  To install SLSsteam, run the headcrab installer:"
    echo ""
    echo -e "  ${CYAN}curl -fsSL https://raw.githubusercontent.com/Deadboy666/h3adcr-b/main/headcrab.sh | bash${NC}"
    echo ""
    echo -e "  This will set up SLSsteam for playing additional content."
    echo ""
}

# Main
main() {
    print_banner
    
    # Install BoilerRoom
    install_boilerroom
    
    print_summary
}

main "$@"
