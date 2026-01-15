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
    release_info=$(curl -fsSL "https://api.github.com/repos/$GITHUB_REPO/releases" 2>/dev/null || echo "")
    
    local version="v1.4.0"  # Fallback version
    local download_url=""
    
    if [ -n "$release_info" ]; then
        # Get first (most recent) release tag
        version=$(echo "$release_info" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')
        download_url=$(echo "$release_info" | grep '"browser_download_url"' | grep -i "appimage" | head -1 | sed 's/.*"browser_download_url": *"\([^"]*\)".*/\1/')
    fi
    
    # Fallback URL if API parsing failed
    if [ -z "$download_url" ]; then
        # Check if we were able to get a version but missed the URL
        if [ "$version" != "v1.4.0" ]; then
             warn "Could not find AppImage in release $version assets. Using fallback."
        fi
        download_url="https://github.com/$GITHUB_REPO/releases/download/$version/$APPIMAGE_NAME"
    fi
    
    info "Downloading BoilerRoom ($version) AppImage from GitHub..."
    info "URL: $download_url"
    
    if ! curl -fL -o "$INSTALL_DIR/$BINARY_NAME" "$download_url"; then
        error "Failed to download BoilerRoom from $download_url"
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

# Install external tools (DepotDownloaderMod, Steamless)
install_tools() {
    local TOOLS_DIR="$HOME/.local/share/boilerroom/tools"
    local SETTINGS_DIR="$HOME/.local/share/com.boilerroom.app"
    local SETTINGS_FILE="$SETTINGS_DIR/settings.json"
    
    step "Checking for .NET runtime..."
    if command -v dotnet &>/dev/null; then
        # Extract version from dotnet --info (more reliable than --version)
        DOTNET_VERSION=$(dotnet --info 2>/dev/null | grep -E "^\s*Version:" | head -1 | awk '{print $2}')
        if [ -z "$DOTNET_VERSION" ]; then
            DOTNET_VERSION=$(dotnet --info 2>/dev/null | grep -E "Host:" -A1 | grep "Version:" | awk '{print $2}')
        fi
        
        # Check major version (need 9+)
        DOTNET_MAJOR=$(echo "$DOTNET_VERSION" | cut -d. -f1)
        if [ -n "$DOTNET_MAJOR" ] && [ "$DOTNET_MAJOR" -ge 9 ] 2>/dev/null; then
            success ".NET $DOTNET_VERSION detected (version 9+ required ✓)"
            DOTNET_AVAILABLE=true
        elif [ -n "$DOTNET_MAJOR" ]; then
            warn ".NET $DOTNET_VERSION detected but version 9+ is required"
            DOTNET_AVAILABLE=false
        else
            warn ".NET found but couldn't determine version"
            DOTNET_AVAILABLE=false
        fi
    else
        warn "No .NET runtime found - will use native binaries where available"
        DOTNET_AVAILABLE=false
    fi
    
    if ask "Download external tools (DepotDownloaderMod, Steamless)?" "y"; then
        step "Downloading tools from BoilerRoom repository..."
        mkdir -p "$TOOLS_DIR"
        
        # Base URL for raw GitHub content
        TOOLS_BASE_URL="https://raw.githubusercontent.com/uxandai/boilerroom/main/tools"
        
        if [ "$DOTNET_AVAILABLE" = true ]; then
            # Download .NET DLL versions
            info "Downloading .NET versions (ddm-net, steamless-net)..."
            
            # DepotDownloaderMod .NET version
            mkdir -p "$TOOLS_DIR/ddm-net"
            for file in DepotDownloaderMod.dll DepotDownloaderMod.runtimeconfig.json DepotDownloaderMod.deps.json \
                        SteamKit2.dll protobuf-net.dll protobuf-net.Core.dll QRCoder.dll ZstdSharp.dll System.IO.Hashing.dll; do
                curl -fsSL "$TOOLS_BASE_URL/ddm-net/$file" -o "$TOOLS_DIR/ddm-net/$file" 2>/dev/null || true
            done
            if [ -f "$TOOLS_DIR/ddm-net/DepotDownloaderMod.dll" ]; then
                success "DepotDownloaderMod.dll downloaded"
                DDM_PATH="$TOOLS_DIR/ddm-net/DepotDownloaderMod.dll"
            else
                warn "Failed to download DepotDownloaderMod.dll"
                DDM_PATH=""
            fi
            
            # Steamless CLI .NET version
            mkdir -p "$TOOLS_DIR/steamless-net"
            for file in Steamless.CLI.dll Steamless.CLI.runtimeconfig.json Steamless.CLI.dll.config; do
                curl -fsSL "$TOOLS_BASE_URL/steamless-net/$file" -o "$TOOLS_DIR/steamless-net/$file" 2>/dev/null || true
            done
            # Also get Plugins folder
            mkdir -p "$TOOLS_DIR/steamless-net/Plugins"
            # Note: Plugins are optional, main CLI should work without them for basic unpacking
            if [ -f "$TOOLS_DIR/steamless-net/Steamless.CLI.dll" ]; then
                success "Steamless.CLI.dll downloaded"
                SL_PATH="$TOOLS_DIR/steamless-net/Steamless.CLI.dll"
            else
                warn "Failed to download Steamless.CLI.dll"
                SL_PATH=""
            fi
        else
            # Download native Linux binary from boilerroom repo
            info "Downloading native Linux DepotDownloaderMod..."
            
            mkdir -p "$TOOLS_DIR/ddm-native"
            if curl -fsSL "$TOOLS_BASE_URL/DepotDownloaderMod" -o "$TOOLS_DIR/ddm-native/DepotDownloaderMod" 2>/dev/null; then
                chmod +x "$TOOLS_DIR/ddm-native/DepotDownloaderMod"
                success "DepotDownloaderMod (native Linux) downloaded"
                DDM_PATH="$TOOLS_DIR/ddm-native/DepotDownloaderMod"
            else
                warn "Native DepotDownloaderMod not available - install .NET 9+ or download manually"
                DDM_PATH=""
            fi
            
            info "Skipping Steamless (.NET not available, use Wine with Steamless.exe manually)"
            SL_PATH=""
        fi
        
        # Configure settings.json with tool paths
        step "Configuring settings..."
        mkdir -p "$SETTINGS_DIR"
        
        if [ -f "$SETTINGS_FILE" ]; then
            # Update existing settings.json
            info "Updating existing settings.json..."
            # Use a simple approach - create temp file and merge
            TEMP_SETTINGS=$(mktemp)
            if command -v jq &>/dev/null; then
                jq --arg ddm "$DDM_PATH" --arg sl "$SL_PATH" \
                    '.toolSettings.depotDownloaderPath = $ddm | .toolSettings.steamlessPath = $sl' \
                    "$SETTINGS_FILE" > "$TEMP_SETTINGS" 2>/dev/null && mv "$TEMP_SETTINGS" "$SETTINGS_FILE"
            else
                # Fallback: just note the paths
                info "Install jq for automatic settings update, or configure paths manually in app"
            fi
        else
            # Create new settings.json
            info "Creating settings.json..."
            cat > "$SETTINGS_FILE" << SETTINGS_EOF
{
  "connectionMode": "local",
  "toolSettings": {
    "depotDownloaderPath": "$DDM_PATH",
    "steamlessPath": "$SL_PATH",
    "slssteamPath": "",
    "steamGridDbApiKey": "",
    "steamApiKey": "",
    "steamUserId": ""
  }
}
SETTINGS_EOF
        fi
        
        success "Tools configured!"
        if [ -n "$DDM_PATH" ]; then
            info "DepotDownloaderMod: $DDM_PATH"
        fi
        if [ -n "$SL_PATH" ]; then
            info "Steamless CLI: $SL_PATH"
        fi
    fi
}

# Main
main() {
    print_banner
    
    # Install BoilerRoom
    install_boilerroom
    
    # Optional: Download and configure external tools
    install_tools
    
    print_summary
}

main "$@"
