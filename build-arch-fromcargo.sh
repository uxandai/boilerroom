#!/usr/bin/env bash
# Build Arch package from pre-compiled cargo binary
# Bypasses makepkg environment which has ring linker issues on CachyOS

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
PKGNAME="boilerroom"
ARCH="x86_64"
PKG_FILENAME="${PKGNAME}-${VERSION}-1-${ARCH}.pkg.tar.zst"

echo -e "${CYAN}Building Arch Package v${VERSION} (from cargo)${NC}"

# Check dependencies
step "Checking build dependencies..."
for cmd in npm cargo zstd tar; do
    if ! command -v $cmd &>/dev/null; then
        error "$cmd is required but not installed"
    fi
done
success "All dependencies found"

mkdir -p "$OUTPUT_DIR"

# Clean previous builds to force asset embedding
step "Cleaning previous builds..."
rm -rf "$SOURCE_DIR/src-tauri/target/release/boilerroom"
rm -rf "$SOURCE_DIR/dist"

# Build Tauri binary using Tauri CLI (ensures assets are embedded correctly)
# This will automatically run 'npm run build' defined in tauri.conf.json
step "Building Tauri binary..."
cd "$SOURCE_DIR"
# --no-bundle means it only builds the binary, skips AppImage/Deb creation
npx tauri build --no-bundle
success "Binary built"

# Define binary path (Tauri CLI puts it in target/release)
BINARY="$SOURCE_DIR/src-tauri/target/release/$PKGNAME"
if [ ! -f "$BINARY" ]; then
    error "Binary not found at $BINARY"
fi

# Create package structure
step "Creating package structure..."
PKG_ROOT="$OUTPUT_DIR/pkg-build"
rm -rf "$PKG_ROOT"
mkdir -p "$PKG_ROOT/usr/bin"
mkdir -p "$PKG_ROOT/usr/share/applications"
mkdir -p "$PKG_ROOT/usr/share/icons/hicolor/32x32/apps"
mkdir -p "$PKG_ROOT/usr/share/icons/hicolor/64x64/apps"
mkdir -p "$PKG_ROOT/usr/share/icons/hicolor/128x128/apps"
mkdir -p "$PKG_ROOT/usr/share/icons/hicolor/256x256/apps"

# Copy binary
cp "$BINARY" "$PKG_ROOT/usr/bin/$PKGNAME"
chmod 755 "$PKG_ROOT/usr/bin/$PKGNAME"

# Copy icons
cp "$SOURCE_DIR/src-tauri/icons/32x32.png" "$PKG_ROOT/usr/share/icons/hicolor/32x32/apps/$PKGNAME.png"
cp "$SOURCE_DIR/src-tauri/icons/64x64.png" "$PKG_ROOT/usr/share/icons/hicolor/64x64/apps/$PKGNAME.png"
cp "$SOURCE_DIR/src-tauri/icons/128x128.png" "$PKG_ROOT/usr/share/icons/hicolor/128x128/apps/$PKGNAME.png"
cp "$SOURCE_DIR/src-tauri/icons/128x128@2x.png" "$PKG_ROOT/usr/share/icons/hicolor/256x256/apps/$PKGNAME.png" 2>/dev/null || true

# Create desktop entry with WebKit workaround
cat > "$PKG_ROOT/usr/share/applications/$PKGNAME.desktop" << EOF
[Desktop Entry]
Name=BoilerRoom
Comment=Steam Deck game manager
Exec=env WEBKIT_DISABLE_COMPOSITING_MODE=1 /usr/bin/$PKGNAME
Icon=$PKGNAME
Terminal=false
Type=Application
Categories=Game;Utility;
Keywords=Steam;Deck;Games;Manager;
StartupWMClass=BoilerRoom
EOF

# Create .PKGINFO
mkdir -p "$PKG_ROOT/.PKGINFO_DIR"
cat > "$PKG_ROOT/.PKGINFO" << EOF
pkgname = $PKGNAME
pkgver = $VERSION-1
pkgdesc = Steam Deck game manager
url = https://github.com/uxandai/boilerroom
builddate = $(date +%s)
packager = BoilerRoom Build Script
size = $(du -sb "$PKG_ROOT" | cut -f1)
arch = $ARCH
depend = webkit2gtk
depend = gtk3
depend = libayatana-appindicator
depend = zstd
EOF

# Create .MTREE (simplified)
step "Creating package archive..."
cd "$PKG_ROOT"

# Create tar.zst archive
tar --zstd -cf "$OUTPUT_DIR/$PKG_FILENAME" \
    --exclude='.PKGINFO_DIR' \
    .PKGINFO \
    usr/

# Cleanup
rm -rf "$PKG_ROOT"

PKG_SIZE=$(du -h "$OUTPUT_DIR/$PKG_FILENAME" | cut -f1)

echo ""
echo -e "${GREEN}╔═══════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║           📦 Arch Package Built!                      ║${NC}"
echo -e "${GREEN}╠═══════════════════════════════════════════════════════╣${NC}"
echo -e "${GREEN}║                                                       ║${NC}"
echo -e "${GREEN}║  Version: v${VERSION}${NC}"
echo -e "${GREEN}║  File: $PKG_FILENAME${NC}"
echo -e "${GREEN}║  Size: $PKG_SIZE${NC}"
echo -e "${GREEN}║  Location: $OUTPUT_DIR${NC}"
echo -e "${GREEN}║                                                       ║${NC}"
echo -e "${GREEN}║  Install with:                                        ║${NC}"
echo -e "${GREEN}║  sudo pacman -U dist/$PKG_FILENAME${NC}"
echo -e "${GREEN}║                                                       ║${NC}"
echo -e "${GREEN}╚═══════════════════════════════════════════════════════╝${NC}"
