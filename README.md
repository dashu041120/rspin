# rspin

A desktop sticky image viewer for Wayland compositors.

## Features

- Always-on-top Wayland overlay window implemented with `wlr-layer-shell`
- GPU rendering via `wgpu` with a CPU fallback that shares the same layer-shell surface
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

## Installation

```bash
cargo build --release
# The binary will be at target/release/rspin
```

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

```
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

| Action | Control |
|--------|---------|
| Move window | Drag with left mouse button |
| Resize window | Drag edges or corners |
| Adjust opacity | Scroll wheel |
| Close | Double-click, Escape, Q, or right-click menu |
| Context menu | Right-click |
| Copy to clipboard | Via right-click menu |

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
- Only a handful of mipmap levels are produced (at most 4, stopping once the texture drops below 512 px) to reduce RAM.
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
