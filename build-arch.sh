#!/usr/bin/env bash
# Build Arch Linux package (pkg.tar.zst) for BoilerRoom
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
BUILD_DIR="$SCRIPT_DIR/dist/arch-build"
OUTPUT_DIR="$SCRIPT_DIR/dist"

# Check dependencies
step "Checking build dependencies..."
for cmd in makepkg npm cargo; do
    if ! command -v $cmd &>/dev/null; then
        error "$cmd is required but not installed"
    fi
done
success "All dependencies found"

# Prepare build directory
step "Preparing build directory..."
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"
mkdir -p "$OUTPUT_DIR"

# Create source tarball
step "Creating source tarball..."
PKGNAME="boilerroom"
PKGVER="1.0.0"

cd "$SOURCE_DIR"
tar --transform "s,^,$PKGNAME-$PKGVER/," \
    --exclude='node_modules' \
    --exclude='target' \
    --exclude='dist' \
    --exclude='.git' \
    -czf "$BUILD_DIR/$PKGNAME-$PKGVER.tar.gz" .

success "Source tarball created"

# Copy PKGBUILD
cp "$SOURCE_DIR/PKGBUILD" "$BUILD_DIR/"

# Build package
step "Building Arch package..."
cd "$BUILD_DIR"

# Update PKGBUILD source path
sed -i "s|source=(.*)|source=(\"$PKGNAME-$PKGVER.tar.gz\")|" PKGBUILD

# CachyOS/Arch linker fix: force use of ld.bfd instead of mold/lld
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="gcc"
export CC="gcc"
export CXX="g++"
export RUSTFLAGS="-C linker=gcc -C link-arg=-fuse-ld=bfd"

# Use system zstd library via pkg-config
export ZSTD_SYS_USE_PKG_CONFIG=1

makepkg -sf --noconfirm

# Copy output
step "Copying package to dist..."
cp "$BUILD_DIR"/*.pkg.tar.zst "$OUTPUT_DIR/" 2>/dev/null || \
cp "$BUILD_DIR"/*.pkg.tar.xz "$OUTPUT_DIR/" 2>/dev/null || \
error "No package file found"

PKG_FILE=$(ls "$OUTPUT_DIR"/*.pkg.tar.* 2>/dev/null | head -1)

echo ""
echo -e "${GREEN}╔═══════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║              📦 Arch Package Built!                   ║${NC}"
echo -e "${GREEN}╠═══════════════════════════════════════════════════════╣${NC}"
echo -e "${GREEN}║                                                       ║${NC}"
echo -e "${GREEN}║  Package: $(basename "$PKG_FILE")${NC}"
echo -e "${GREEN}║  Location: $OUTPUT_DIR${NC}"
echo -e "${GREEN}║                                                       ║${NC}"
echo -e "${GREEN}║  Install with:                                        ║${NC}"
echo -e "${GREEN}║  sudo pacman -U $(basename "$PKG_FILE")${NC}"
echo -e "${GREEN}║                                                       ║${NC}"
echo -e "${GREEN}╚═══════════════════════════════════════════════════════╝${NC}"
