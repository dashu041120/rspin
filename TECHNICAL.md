# Technical Notes

This document dives deeper into the implementation details of `rspin`, focusing on rendering pipelines, resource management, and design constraints.

## Rendering Pipelines

### GPU Path (Default)

1. **Wayland Surface**  
   - Created via `smithay-client-toolkit` using the `wlr-layer-shell` protocol.  
   - The layer surface stays always-on-top on the current workspace and respects margin/anchor settings to control position.

2. **wgpu Renderer** (`src/wgpu_renderer.rs`)  
   - Builds a single textured quad (two triangles) with a simple WGSL shader.  
   - The main image texture is uploaded once; on resize the swapchain is reconfigured.  
   - The context menu is rasterized in software, converted to RGBA, and uploaded as a tiny overlay texture. During `render()` the overlay is drawn with a dedicated viewport so only that region is touched—this prevents GPU→CPU fallback when the menu appears.

3. **Opacity & Interaction**  
   - Opacity is passed to the fragment shader via a uniform buffer.  
   - Pointer events (move, resize, menu) are handled in `WaylandApp`, which updates state and requests redraws.

### CPU Path (`--cpu`)

1. **Shared Memory Buffer**  
   - Uses `SlotPool` from `smithay-client-toolkit` to allocate `wl_shm` buffers sized to the current window.  
   - The image is rendered into BGRA memory; a cached scaled version is stored only in CPU mode.

2. **Rendering Strategy**  
   - During active resize a fast nearest-neighbor path is used.  
   - When idle, a bilinear interpolation pass writes into the buffer, and opacity is applied per-pixel.  
   - Context menu (`render_menu`) draws directly into the CPU buffer, utilizing the same emoji-enhanced glyph rendering as the GPU overlay.

## Resource Constraints

- **Initial Size**: Limited to 10 % of the active screen area.  
- **Maximum Size**: Clamped to both the display dimensions and a hard cap (`MAX_SIZE`, currently 4096 px).  
- **Mipmaps**: Only up to four levels are generated, stopping once the texture drops below 512 px, avoiding runaway memory usage for large inputs.  
- **Overlay Region**: Menu overlay textures are trimmed to the menu rectangle to avoid uploading the entire framebuffer each time.

## Modules at a Glance

| File | Responsibility |
|------|----------------|
| `src/cli.rs` | Defines command line interface (`--cpu`, opacity, scale, positioning). |
| `src/image_loader.rs` | Decodes images via the `image` crate, converts to BGRA, and produces limited mipmaps. |
| `src/wayland.rs` | Core event loop, input handling, resizing logic, menu state, CPU rendering, and GPU overlay coordination. |
| `src/wgpu_renderer.rs` | Manages `wgpu` device/swapchain, texture uploads, uniform buffers, and menu overlay rendering. |
| `src/main.rs` | Wires CLI parsing, image loading, and Wayland startup. |

## Key Dependencies

- **Wayland stack**: `smithay-client-toolkit`, `wayland-client`, `wayland-protocols`.  
- **Rendering**: `wgpu`, `raw-window-handle`, `pollster`, `bytemuck`.  
- **Images**: `image` crate with default format support.  
- **CLI / logging**: `clap`, `anyhow`, `log`, `env_logger`.  
- Refer to `Cargo.toml` for versions and optional features.

## Tips for Extending

- Keep GPU/CPU paths feature-parallel: when adding visual changes, update both `render_menu` (CPU) and the overlay builder in `WaylandApp::update_gpu_menu_overlay`.
- Bounds checking is critical; always clamp window sizes before allocating buffers to avoid Wayland protocol errors.
- When touching the GPU pipeline, test on multiple compositors to ensure the layer-shell behavior and transparency remain correct.
- Use `RUST_LOG=info` (or `debug`) to inspect resizing, clipboard operations, and Wayland events during development.

This file complements `DEVELOPMENT.md`, which covers workflow and high-level architecture.
