#!/bin/bash
# Local packaging script for development and testing

set -e

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

echo "==> Building rspin packages locally..."

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Build release binary
echo -e "${BLUE}==> Building release binary...${NC}"
cargo build --release

# Create output directory
mkdir -p dist
rm -rf dist/*

# Build DEB package
if command -v cargo-deb &> /dev/null; then
    echo -e "${BLUE}==> Building DEB package...${NC}"
    cargo deb --output dist/
    echo -e "${GREEN}✓ DEB package created${NC}"
else
    echo "cargo-deb not installed. Install with: cargo install cargo-deb"
fi

# Build RPM package
if command -v cargo-generate-rpm &> /dev/null; then
    echo -e "${BLUE}==> Building RPM package...${NC}"
    cargo generate-rpm --output dist/
    echo -e "${GREEN}✓ RPM package created${NC}"
else
    echo "cargo-generate-rpm not installed. Install with: cargo install cargo-generate-rpm"
fi

# Build Arch package (requires makepkg on Arch Linux)
if command -v makepkg &> /dev/null; then
    echo -e "${BLUE}==> Building Arch package...${NC}"
    cd packaging/arch
    cp PKGBUILD.local PKGBUILD.build
    makepkg -f --config PKGBUILD.build
    mv *.pkg.tar.zst ../../dist/ 2>/dev/null || true
    rm -f PKGBUILD.build
    cd "$PROJECT_ROOT"
    echo -e "${GREEN}✓ Arch package created${NC}"
else
    echo "makepkg not available (requires Arch Linux)"
fi

# Build tar.gz portable package
echo -e "${BLUE}==> Building portable tar.gz package...${NC}"
TARBALL_NAME="rspin-${CARGO_PKG_VERSION:-0.1.1}-x86_64-linux"
TARBALL_DIR="dist/$TARBALL_NAME"
rm -rf "$TARBALL_DIR"
mkdir -p "$TARBALL_DIR"

# Copy files
cp target/release/rspin "$TARBALL_DIR/"
cp README.md "$TARBALL_DIR/"
cp LICENSE "$TARBALL_DIR/"
cp scripts/install.sh "$TARBALL_DIR/"
cp scripts/uninstall.sh "$TARBALL_DIR/"

# Create tarball
cd dist
tar czf "${TARBALL_NAME}.tar.gz" "$TARBALL_NAME"
rm -rf "$TARBALL_NAME"
cd "$PROJECT_ROOT"
echo -e "${GREEN}✓ Portable package created${NC}"

echo ""
echo -e "${GREEN}==> Packages created in dist/ directory:${NC}"
ls -lh dist/

echo ""
echo -e "${BLUE}To install:${NC}"
echo "  Debian/Ubuntu: sudo dpkg -i dist/*.deb"
echo "  Fedora/RHEL:   sudo rpm -i dist/*.rpm"
echo "  Arch Linux:    sudo pacman -U dist/*.pkg.tar.zst"
echo "  Portable:      tar xzf dist/*.tar.gz && cd rspin-* && ./install.sh"
