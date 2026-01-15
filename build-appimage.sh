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

# Build frontend
step "Building frontend..."
cd "$SOURCE_DIR"
npm ci
npm run build
success "Frontend built"

# Build Tauri binary (without bundling)
step "Building Tauri binary..."
cd "$SOURCE_DIR/src-tauri"

# Clean to remove cached warnings
cargo clean -p boilerroom 2>/dev/null || true

# Build release binary
cargo build --release
success "Binary built"

# Create AppImage using appimagetool
step "Creating AppImage with appimagetool..."

APPDIR="$SOURCE_DIR/src-tauri/target/release/bundle/appimage/BoilerRoom.AppDir"

# Check if AppDir exists (Tauri creates it during build attempt)
if [ ! -d "$APPDIR" ]; then
    # Try to create it by running tauri build (it will fail at linuxdeploy but create AppDir)
    npm run tauri build -- --bundles appimage 2>/dev/null || true
fi

if [ ! -d "$APPDIR" ]; then
    error "AppDir not found at $APPDIR"
fi

cd "$APPDIR"

# Fix icon naming (BoilerRoom.png -> boilerroom.png)  
if [ -f "BoilerRoom.png" ] && [ ! -f "boilerroom.png" ]; then
    cp BoilerRoom.png boilerroom.png
fi

# Build AppImage
ARCH=x86_64 appimagetool . "../$APPIMAGE_NAME"

# Copy to dist
cp "../$APPIMAGE_NAME" "$OUTPUT_DIR/"

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
