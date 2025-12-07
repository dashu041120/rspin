# Development Guide

This document is intended for contributors and IDE / AI tooling that need a deeper understanding of `rspin`. It describes the core modules, external libraries, and relevant commands.

## Architecture Overview

```
CLI (clap) ──> image_loader (image crate)
                │
                ├─> GPU path (wgpu + layer-shell surface)
                └─> CPU path (wl_shm buffer on the same layer-shell surface)
```

- **CLI / args parsing (`src/cli.rs`)** – implemented with [`clap`](https://crates.io/crates/clap). Supports reading from stdin, scaling, opacity, and a `--cpu` flag to disable GPU rendering.
- **Image loading (`src/image_loader.rs`)** – uses the [`image`](https://crates.io/crates/image) crate to decode files or stdin buffers into BGRA data and generates a limited set of mipmaps.
- **Wayland integration (`src/wayland.rs`)** – built directly on [`smithay-client-toolkit`](https://crates.io/crates/smithay-client-toolkit). Creates a `wlr-layer-shell` surface, handles inputs (pointer, keyboard), and manages resizing / positioning logic.
- **GPU renderer (`src/wgpu_renderer.rs`)** – employs [`wgpu`](https://crates.io/crates/wgpu) to render the decoded texture. A small overlay texture is used for the context menu so the GPU path stays active even when the menu is open.
- **CPU fallback** – when `--cpu` is specified (or GPU init fails), rendering occurs via a shared-memory buffer (`wl_shm`). The same menu drawing routine is shared by both paths.

## External Libraries

| Purpose | Crate |
|---------|-------|
| Wayland protocol bindings | `smithay-client-toolkit`, `wayland-client`, `wayland-protocols` |
| GPU backend | `wgpu`, `raw-window-handle`, `pollster`, `bytemuck` |
| Image decoding | `image` |
| CLI and logging | `clap`, `anyhow`, `thiserror`, `log`, `env_logger` |
| Misc | `atty` (stdin detection), `memmap2` (slot pool utilities) |

The exact versions are listed in `Cargo.toml`.

## Rendering Details

- The GPU path draws a single textured quad. During resizing `wgpu_renderer::resize` reconfigures the swapchain, and `render()` composes the base texture plus a context-menu overlay using viewports.
- The context menu is rasterized into a local BGRA buffer, converted to RGBA, and uploaded through `update_overlay_texture`. No CPU fallback is required for menus anymore.
- CPU rendering uses `SlotPool` from `smithay-client-toolkit` to allocate wl_shm buffers. A cached scaled image is maintained only when running in CPU mode to avoid duplicating data alongside the GPU.
- Mipmaps: only up to four levels are generated, and generation stops once the texture drops below 512 px. This keeps RAM usage predictable even for large images.
- The initial window size is clamped to 10 % of the current screen area and never expands beyond 100 % of that screen. This prevents over-allocating GPU or CPU buffers.

## Development Workflow

Common commands:

```bash
# Format sources
cargo fmt

# Lint
cargo clippy --all-targets -- -D warnings

# Run in debug mode
cargo run -- image.png

# Release build
cargo build --release
```

When working with Wayland protocol changes, ensure the compositor you test on exposes `wlr-layer-shell`. `rspin` does not depend on `winit`; everything happens on the raw Wayland connection, so debugging Wayland events can be done by enabling `RUST_LOG=wayland_client=debug`.

## File Map

- `src/cli.rs` – argument parsing and stdin helpers.
- `src/image_loader.rs` – decoding, scaling, and mipmap generation helpers.
- `src/wayland.rs` – main event loop, input handling, menu logic, and CPU path.
- `src/wgpu_renderer.rs` – GPU renderer and overlay helpers.
- `src/main.rs` – glue code that wires CLI parsing, image loading, and Wayland startup.

## Contributing Tips

- Keep CPU and GPU paths in sync; most UI changes should be implemented in `WaylandApp::render_menu` so both renderers stay consistent.
- Test both GPU and CPU modes before submitting changes (`rspin image.png` vs `rspin --cpu image.png`).
- For clipboard features, the external binaries `wl-copy` or `xclip` are invoked via `std::process::Command`. Make sure to handle errors gracefully if they are missing.

Happy hacking!
