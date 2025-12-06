# rspin

A desktop sticky image viewer for Wayland compositors.

## Features

- Display images in a floating, always-on-top overlay window
- Support for multiple image formats (PNG, JPEG, GIF, WebP, BMP, etc.)
- Customizable opacity (transparency)
- Image scaling
- Input from file path or stdin pipe
- **Interactive controls**:
  - Drag to move window
  - Drag edges/corners to resize
  - Scroll wheel to adjust opacity
  - Double-click to close
  - Right-click context menu
  - Copy image to clipboard (via wl-copy or xclip)
- Auto-limits initial size to 20% of screen

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

### Options

```
Usage: rspin [OPTIONS] [IMAGE]

Arguments:
  [IMAGE]  Path to the image file (can also be provided via stdin pipe)

Options:
  -o, --opacity <OPACITY>    Opacity of the window (0.0 - 1.0) [default: 1.0]
  -x, --pos-x <POS_X>        Initial X position of the window
  -y, --pos-y <POS_Y>        Initial Y position of the window
  -s, --scale <SCALE>        Scale factor for the image [default: 1.0]
      --gpu                  Use GPU rendering (default: true)
      --no-gpu               Use CPU rendering with layer-shell
      --app-id <APP_ID>      Custom app-id for window rules [default: rspin]
  -h, --help                 Print help
  -V, --version              Print version
```

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

## Floating Overlay Window

rspin supports two modes for creating floating overlay windows:

### GPU Mode (Default) - For niri compositor

Uses **winit + wgpu** for GPU-accelerated rendering. Requires a window rule in niri config:

```kdl
window-rule {
    match app-id="^rspin$"
    open-floating true
}
```

Run with: `rspin image.png` (GPU is default)

**Note**: Floating windows in niri only stay on top **within the current workspace**. When you switch workspaces, the window stays in its original workspace.

### CPU Mode - For global overlay across all workspaces

Uses **wlr-layer-shell** protocol to create a **true global overlay**:
- Visible on **all workspaces** (global layer)
- Always stays on top of all windows
- Doesn't appear in taskbar or window lists
- Can be positioned anywhere, even partially off-screen

Run with: `rspin --no-gpu image.png`

**Use this mode if you need the image to stay visible when switching workspaces.**

## Performance Optimizations

- **Mipmap Generation**: Automatically generates progressively half-sized versions (up to 8 levels)
- **Smart Level Selection**: Chooses optimal mipmap level based on display size
- **Fast Resize Preview**: Optimized nearest-neighbor scaling during active resizing
- **Frame Rate Limiting**: Limits redraws to ~40fps during resize
- **High-Quality Final Render**: Bilinear interpolation when resize completes
- **Cached Rendering**: Caches scaled results to avoid redundant computation
- **Buffer Size Limits**: Automatically limits window size to prevent memory errors (max 4096x4096)

## Supported Image Formats

- PNG
- JPEG
- GIF
- WebP
- BMP
- ICO
- TIFF

## License

MIT
