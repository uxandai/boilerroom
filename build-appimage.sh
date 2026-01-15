#!/usr/bin/env bash
# Build BoilerRoom AppImage
# Run from repository root

set -eu

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

step() { echo -e "\n${CYAN}▶ $1${NC}"; }
success() { echo -e "  ${GREEN}✅ $1${NC}"; }
error() { echo -e "  ${RED}❌ $1${NC}"; exit 1; }

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SOURCE_DIR="$SCRIPT_DIR/source"
OUTPUT_DIR="$SCRIPT_DIR/dist"

# Extract version from Cargo.toml
VERSION=$(grep '^version' "$SOURCE_DIR/src-tauri/Cargo.toml" | head -1 | sed 's/.*"\([^"]*\)".*/\1/')
APPIMAGE_NAME="BoilerRoom_${VERSION}_amd64.AppImage"

echo -e "${CYAN}Building BoilerRoom v${VERSION}${NC}"

# Check dependencies
step "Checking build dependencies..."
for cmd in npm cargo appimagetool; do
    if ! command -v $cmd &>/dev/null; then
        error "$cmd is required but not installed"
    fi
done
success "All dependencies found"

# Prepare output directory
mkdir -p "$OUTPUT_DIR"

# Clean previous builds
step "Cleaning previous builds..."
rm -rf "$SOURCE_DIR/src-tauri/target/release/boilerroom"
rm -rf "$SOURCE_DIR/dist"

# Build Tauri binary (ensure assets are embedded via Tauri CLI)
step "Building Tauri binary..."
cd "$SOURCE_DIR"
# --no-bundle means it only builds the binary, skips AppImage/Deb creation
npx tauri build --no-bundle
success "Binary built"

BINARY="$SOURCE_DIR/src-tauri/target/release/boilerroom"
if [ ! -f "$BINARY" ]; then
    error "Binary not found at $BINARY"
fi

# Create AppDir structure manually
step "Creating AppDir structure..."
APPDIR="$OUTPUT_DIR/AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"
mkdir -p "$APPDIR/usr/share/icons/hicolor/32x32/apps"
mkdir -p "$APPDIR/usr/share/icons/hicolor/128x128/apps"
mkdir -p "$APPDIR/usr/share/icons/hicolor/256x256/apps"
mkdir -p "$APPDIR/usr/share/applications"

# Copy binary
cp "$BINARY" "$APPDIR/usr/bin/boilerroom"

# Copy icons
cp "$SOURCE_DIR/src-tauri/icons/32x32.png" "$APPDIR/usr/share/icons/hicolor/32x32/apps/boilerroom.png"
cp "$SOURCE_DIR/src-tauri/icons/128x128.png" "$APPDIR/usr/share/icons/hicolor/128x128/apps/boilerroom.png"
cp "$SOURCE_DIR/src-tauri/icons/128x128@2x.png" "$APPDIR/usr/share/icons/hicolor/256x256/apps/boilerroom.png" 2>/dev/null || true
cp "$SOURCE_DIR/src-tauri/icons/128x128.png" "$APPDIR/.DirIcon" # AppImage icon
cp "$SOURCE_DIR/src-tauri/icons/128x128.png" "$APPDIR/boilerroom.png" # Required by appimagetool validation

# Create desktop file
cat > "$APPDIR/boilerroom.desktop" << EOF
[Desktop Entry]
Name=BoilerRoom
Comment=Steam Deck game manager
Exec=boilerroom
Icon=boilerroom
Type=Application
Categories=Game;Utility;
EOF
cp "$APPDIR/boilerroom.desktop" "$APPDIR/usr/share/applications/boilerroom.desktop"

# Create AppRun symlink
ln -s usr/bin/boilerroom "$APPDIR/AppRun"

# Create AppImage using appimagetool
step "Creating AppImage with appimagetool..."
cd "$OUTPUT_DIR"
ARCH=x86_64 appimagetool "$APPDIR" "$APPIMAGE_NAME"

# Cleanup AppDir
rm -rf "$APPDIR"

echo ""
echo -e "${GREEN}╔═══════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║              📦 AppImage Built!                       ║${NC}"
echo -e "${GREEN}╠═══════════════════════════════════════════════════════╣${NC}"
echo -e "${GREEN}║                                                       ║${NC}"
echo -e "${GREEN}║  Version: v${VERSION}${NC}"
echo -e "${GREEN}║  File: $APPIMAGE_NAME${NC}"
echo -e "${GREEN}║  Location: $OUTPUT_DIR${NC}"
echo -e "${GREEN}║                                                       ║${NC}"
echo -e "${GREEN}╚═══════════════════════════════════════════════════════╝${NC}"
