# rspin

A desktop sticky image viewer for Wayland compositors.

<img style="max-width: 36%; height: auto;" alt="image" src="https://github.com/user-attachments/assets/32d18b7f-3aec-48d3-aeaa-51c87945282a" />


## Table of Contents

- [Features](#features)
- [Requirements](#requirements)
- [Usage](#usage)
- [Configuration for niri](#configuration-for-niri)
- [Controls](#controls)
- [Installation](#installation)
- [Context Menu Options](#context-menu-options)
- [Scaling Modes](#scaling-modes)
- [Wayland overlay mode](#wayland-overlay-mode)
- [Rendering details](#rendering-details)
- [Supported Image Formats](#supported-image-formats)
- [Development](#development)
- [License](#license)

## Features

- Always-on-top Wayland overlay window implemented with `wlr-layer-shell`
- GPU rendering via `wgpu` with a CPU fallback that shares the same layer-shell surface
- Deferred GPU initialization so the first frame appears instantly even when GPU mode is enabled
- Context menu rendered directly on the GPU (no more CPU fallback/blur when it is open)
- Auto-limits the initial size to **10% of the screen area** and never allows scaling beyond 100% of the active display
- Transparent window with scroll-wheel opacity control
- Input from file path or stdin pipe
- Emoji-enriched context menu with quick actions (close, copy, opacity ±, scale toggle)
- Copy-to-clipboard using `wl-copy` or `xclip`
- Supports PNG, JPEG, GIF, WebP, BMP, ICO, TIFF out of the box through the `image` crate

## Requirements

- Wayland compositor with wlr-layer-shell support (niri, sway, hyprland, etc.)
- Rust 1.70+
- Optional: `wl-copy` or `xclip` for clipboard support

## Usage

### From file path

```bash
rspin image.png
rspin screenshot.jpg --opacity 0.8
rspin large-image.png --scale 0.5
```

### From stdin (pipe)

```bash
cat image.png | rspin
grim -g "$(slurp)" - | rspin --opacity 0.9
```

### Command line reference

```bash
Usage: rspin [OPTIONS] [IMAGE]

Arguments:
  [IMAGE]  Path to the image file (can also be provided via stdin pipe)

Options:
  -o, --opacity <VALUE>   Window opacity (0.0 - 1.0) [default: 1.0]
  -x, --pos-x <PX>        Initial X position (optional)
  -y, --pos-y <PX>        Initial Y position (optional)
  -s, --scale <FACTOR>    Scale image before displaying [default: 1.0]
      --cpu               Force CPU rendering (GPU is enabled by default)
  -h, --help              Print help
  -V, --version           Print version
```

GPU mode is the default and keeps the entire rendering path on the GPU (including the context menu). Pass `--cpu` if you need the shared-memory renderer instead.

## Configuration for niri

To make rspin windows float in niri, add this to your niri config (`~/.config/niri/config.kdl`):

```kdl
window-rule {
    match app-id="^rspin$"
    open-floating true
}
```

You can also set a custom app-id for more control:

```bash
rspin --app-id my-viewer image.png
```

Then configure it separately:

```kdl
window-rule {
    match app-id="^my-viewer$"
    open-floating true
    default-floating-position x=100 y=100 relative-to="top-left"
    opacity 0.95
}
```

## Controls

| Action            | Control                                      |
| ----------------- | -------------------------------------------- |
| Move window       | Drag with left mouse button                  |
| Resize window     | Drag edges or corners                        |
| Adjust opacity    | Scroll wheel                                 |
| Close             | Double-click, Escape, Q, or right-click menu |
| Context menu      | Right-click                                  |
| Copy to clipboard | Via right-click menu                         |

## Installation

### Pre-built Packages

**Debian/Ubuntu (DEB):**

```bash
# Download from releases page
wget https://github.com/dashu041120/rspin/releases/download/v0.1.1/rspin_0.1.1_amd64.deb
sudo dpkg -i rspin_0.1.1_amd64.deb
```

**Fedora/RHEL (RPM):**

```bash
# Download from releases page
wget https://github.com/dashu041120/rspin/releases/download/v0.1.1/rspin-0.1.1-1.x86_64.rpm
sudo dnf install rspin-0.1.1-1.x86_64.rpm
```

**Arch Linux:**

```bash
# Download from releases page
wget https://github.com/dashu041120/rspin/releases/download/v0.1.1/rspin-0.1.1-1-x86_64.pkg.tar.zst
sudo pacman -U rspin-0.1.1-1-x86_64.pkg.tar.zst
```

**Portable (any Linux):**

```bash
# Download tarball from releases page
wget https://github.com/dashu041120/rspin/releases/download/v0.1.1/rspin-0.1.1-x86_64-linux.tar.gz
tar xzf rspin-0.1.1-x86_64-linux.tar.gz
cd rspin-0.1.1-x86_64-linux
./install.sh  # Installs to ~/.local/bin
```

**NixOS / Nix (Flakes):**

```bash
# Run directly without installing
nix run github:dashu041120/rspin -- image.png

# Run via nix-shell
nix shell github:dashu041120/rspin
# you can also clone and run nix shell
rspin /path/to/image.png

# Install to your profile
nix profile install github:dashu041120/rspin

# Or add to your NixOS configuration or home-manager
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rspin.url = "github:dashu041120/rspin";
  };

  outputs = { self, nixpkgs, rspin, ... }: {
    # For NixOS system configuration
    nixosConfigurations.yourhost = nixpkgs.lib.nixosSystem {
      # ...
      environment.systemPackages = [ rspin.packages.x86_64-linux.default ];
    };

    # Or for home-manager
    homeConfigurations.youruser = home-manager.lib.homeManagerConfiguration {
      # ...
      home.packages = [ rspin.packages.x86_64-linux.default ];
    };
  };
}
```

**Nix (without Flakes):**

```bash
# Clone the repository
git clone https://github.com/dashu041120/rspin.git
cd rspin

# Build and install
nix-env -if .

# Or just build
nix-build

# Run from result
./result/bin/rspin image.png
```

> **Note for Nix users:** After downloading a release, you need to update the `sha256` hash in `flake.nix`.
> You can get the correct hash by running:
>
> ```bash
> nix flake update
> nix build  # This will fail with the correct hash in the error message
> ```
>
> Then update the `sha256` value in `flake.nix` and rebuild.

### Build from Source

```bash
cargo build --release
# The binary will be at target/release/rspin
```

### Building Packages

**DEB package:**

```bash
cargo install cargo-deb
cargo deb
# Output: target/debian/rspin_*.deb
```

**RPM package:**

```bash
cargo install cargo-generate-rpm
cargo build --release
cargo generate-rpm
# Output: target/generate-rpm/rspin-*.rpm
```

**Arch package:**

```bash
cd packaging/arch
makepkg -sf
# Output: rspin-*.pkg.tar.zst
```

**Portable tarball:**

```bash
./scripts/build-tarball.sh
# Output: dist/rspin-*-x86_64-linux.tar.gz
```


## Context Menu Options

- **Close** - Exit the application
- **Copy to Clipboard** - Copy image to clipboard (requires wl-copy or xclip)
- **Opacity +** - Increase opacity by 5%
- **Opacity -** - Decrease opacity by 5%
- **Scale: Free / Scale: Keep Ratio** - Toggle between aspect ratio locked and free scaling modes

## Scaling Modes

When resizing the window:

- **Keep Aspect Ratio** (default): Maintains the original image proportions when resizing from any edge or corner
- **Free Scale**: Allows stretching the image to any dimensions

During resize operations, a fast preview is shown for smooth interaction. High-quality bilinear interpolation is applied when you release the mouse button.

## Wayland overlay mode

`rspin` always uses the Wayland **wlr-layer-shell** protocol via `smithay-client-toolkit`. The GPU backend (`wgpu`) renders directly into the layer surface; when `--cpu` is specified the same surface is painted through a shared-memory buffer. Because the menu is now drawn via a wgpu overlay, the GPU path never has to fall back to CPU just to show UI.

## Rendering details

- Initial size is capped at 10% of the current display area and subsequent resizes are clamped to that display.
- GPU rendering uses a single textured quad drawn via `wgpu`. The context menu is rendered into a small RGBA buffer, uploaded as an overlay texture, and composited with a viewport so that only the menu area is touched.
- CPU rendering uses a `wl_shm` buffer. A cached scaled image is maintained only when running purely on the CPU; GPU mode disables the cache to save memory.
- Memory optimizations: font system is lazy-loaded only when menu opens, image data is released after GPU upload, and texture uploads use chunked streaming to reduce peak memory.
- During live resizing a fast nearest-neighbor path is used, while the steady-state image uses bilinear interpolation plus opacity blending.

## Supported Image Formats

- PNG
- JPEG
- GIF
- WebP
- BMP
- ICO
- TIFF

## Development

See [DEVELOPMENT.md](DEVELOPMENT.md) for an overview of the architecture, dependencies, and tips for contributing or integrating with AI IDE tooling.

## License

MIT
