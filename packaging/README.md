# Packaging

This directory contains packaging configurations for different Linux distributions.

## Automated Builds (GitHub Actions)

Packages are automatically built on GitHub Actions when you push a tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The workflow will build DEB, RPM, and Arch packages and attach them to the GitHub release.

## Local Building

### Prerequisites

Install the required tools:

```bash
# For DEB packages
cargo install cargo-deb

# For RPM packages
cargo install cargo-generate-rpm

# For Arch packages (Arch Linux only)
sudo pacman -S base-devel
```

### Build All Packages

Run the build script:

```bash
./scripts/build-packages.sh
```

Packages will be created in the `dist/` directory.

### Build Individual Packages

**DEB (Debian/Ubuntu):**
```bash
cargo deb --output dist/
```

**RPM (Fedora/RHEL/CentOS):**
```bash
cargo generate-rpm --output dist/
```

**Arch Linux:**
```bash
cd packaging/arch
makepkg -sf
```

**Portable tar.gz:**
```bash
cargo build --release
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
TARBALL_NAME="rspin-${VERSION}-x86_64-linux"
mkdir -p "dist/$TARBALL_NAME"
cp target/release/rspin README.md LICENSE scripts/*.sh "dist/$TARBALL_NAME/"
cd dist && tar czf "${TARBALL_NAME}.tar.gz" "$TARBALL_NAME"
```

## Package Details

### DEB Package
- **Dependencies**: libwayland-client0, libxkbcommon0
- **Suggests**: wl-clipboard or xclip
- **Install location**: /usr/bin/rspin

### RPM Package
- **Dependencies**: wayland, libxkbcommon
- **Install location**: /usr/bin/rspin

### Arch Package
- **Dependencies**: wayland, libxkbcommon
- **Optional**: wl-clipboard, xclip
- **Install location**: /usr/bin/rspin

### Portable tar.gz Package
- **No dependencies** (statically linked where possible)
- **Install location**: ~/.local/bin/rspin (user-local)
- **Includes**: install.sh and uninstall.sh scripts
- **Automatic PATH setup**: Offers to add ~/.local/bin to your PATH

## Installation

**Debian/Ubuntu:**
```bash
sudo dpkg -i dist/rspin_*.deb
# If dependencies are missing:
sudo apt-get install -f
```

**Fedora/RHEL:**
```bash
sudo rpm -i dist/rspin-*.rpm
# Or with dnf:
sudo dnf install dist/rspin-*.rpm
```

**Arch Linux:**
```bash
sudo pacman -U dist/rspin-*.pkg.tar.zst
```

**Portable tar.gz (any Linux):**
```bash
tar xzf rspin-*-x86_64-linux.tar.gz
cd rspin-*-x86_64-linux
./install.sh
```

The install script will:
- Copy the binary to `~/.local/bin/rspin`
- Offer to add `~/.local/bin` to your PATH if needed
- Install documentation to `~/.local/share/doc/rspin`

## Uninstallation

**Debian/Ubuntu:**
```bash
sudo apt remove rspin
```

**Fedora/RHEL:**
```bash
sudo dnf remove rspin
```

**Arch Linux:**
```bash
sudo pacman -R rspin
```

**Portable installation:**
```bash
~/.local/bin/rspin/../uninstall.sh
# Or if you still have the extracted directory:
cd rspin-*-x86_64-linux
./uninstall.sh
```
