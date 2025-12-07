// Wayland integration module
// Handles all Wayland-specific functionality using smithay-client-toolkit

use crate::image_loader::ImageData;
use crate::wgpu_renderer::WgpuRenderer;
use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use cosmic_text::{Attrs, AttrsOwned, Buffer, FontSystem, Metrics, Shaping, SwashCache, Color as TextColor, Family};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
        pointer::{CursorIcon, PointerEvent, PointerEventKind, PointerHandler, ThemedPointer, ThemeSpec},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{
        slot::{Buffer as ShmBuffer, SlotPool},
        Shm, ShmHandler,
    },
};
use std::process::Command;
use std::time::Instant;
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, Proxy, QueueHandle,
};

/// Mouse button constants
const BTN_LEFT: u32 = 272;
const BTN_RIGHT: u32 = 273;

/// Double-click detection threshold in milliseconds
const DOUBLE_CLICK_THRESHOLD_MS: u128 = 300;

/// Resize edge detection margin in pixels
const RESIZE_MARGIN: f64 = 10.0;

/// Minimum window size
const MIN_SIZE: u32 = 50;

/// Maximum window size to prevent buffer allocation failures
const MAX_SIZE: u32 = 4096;

/// Maximum buffer size (64MB to avoid Wayland buffer issues)
const MAX_BUFFER_SIZE: usize = 64 * 1024 * 1024;

/// Opacity adjustment step for scroll wheel
const OPACITY_STEP: f32 = 0.05;

/// Resize direction flags
#[derive(Debug, Clone, Copy, PartialEq)]
enum ResizeEdge {
    None,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Scale mode for resizing
#[derive(Debug, Clone, Copy, PartialEq)]
enum ScaleMode {
    /// Keep aspect ratio when resizing
    KeepAspectRatio,
    /// Free scaling (stretch)
    FreeScale,
}

/// Context menu state
#[derive(Debug, Clone, Copy, PartialEq)]
enum MenuState {
    Hidden,
    Visible,
}

/// Menu item indices
const MENU_ITEM_CLOSE: usize = 0;
const MENU_ITEM_COPY: usize = 1;
const MENU_ITEM_OPACITY_UP: usize = 2;
const MENU_ITEM_OPACITY_DOWN: usize = 3;
const MENU_ITEM_SCALE_MODE: usize = 4;
const MENU_ITEM_HEIGHT: u32 = 25;
const MENU_WIDTH: u32 = 180;

/// Main Wayland application state
struct WaylandApp {
    // Registry state
    registry_state: RegistryState,
    // Seat state for input handling
    seat_state: SeatState,
    // Output state for display info
    output_state: OutputState,
    // Shared memory for buffer allocation
    shm: Shm,
    // Layer shell for overlay windows
    layer_shell: LayerShell,
    // Compositor state
    compositor_state: CompositorState,

    // Wayland display pointer (for GPU rendering)
    display_ptr: *mut std::ffi::c_void,

    // Application-specific state
    image: ImageData,
    opacity: f32,
    should_exit: bool,

    // Display dimensions for size limiting
    display_width: u32,
    display_height: u32,

    // Surface and buffer management
    layer_surface: Option<LayerSurface>,
    pool: Option<SlotPool>,
    buffer: Option<ShmBuffer>,
    width: u32,
    height: u32,
    configured: bool,

    // Window position (margins from top-left)
    margin_left: i32,
    margin_top: i32,

    // Pointer state
    pointer_pos: (f64, f64),
    themed_pointer: Option<ThemedPointer>,
    set_cursor_on_next_frame: Option<CursorIcon>,

    // Dragging state
    dragging: bool,
    drag_start_pos: (f64, f64),
    drag_start_margin: (i32, i32),

    // Resizing state
    resizing: bool,
    resize_edge: ResizeEdge,
    resize_start_pos: (f64, f64),
    resize_start_size: (u32, u32),
    resize_start_margin: (i32, i32),

    // Double-click detection
    last_click_time: Option<Instant>,
    last_click_pos: (f64, f64),

    // Context menu
    menu_state: MenuState,
    menu_pos: (i32, i32),
    menu_hover_item: Option<usize>,

    // Redraw flag
    needs_redraw: bool,

    // Scale mode (keep aspect ratio or free scale)
    scale_mode: ScaleMode,
    // Original image aspect ratio (width / height)
    original_aspect_ratio: f32,
    // Cached scaled image data for performance during resize
    cached_scaled_image: Option<Vec<u8>>,
    cached_scaled_size: (u32, u32),
    // Frame rate limiting for resize
    last_resize_draw: Option<Instant>,

    // GPU rendering
    use_gpu: bool,
    gpu_renderer: Option<WgpuRenderer>,
    gpu_initialized: bool,

    // Text rendering
    font_system: FontSystem,
    swash_cache: SwashCache,
    menu_text_attrs: AttrsOwned,
    menu_text_metrics: Metrics,
}

impl WaylandApp {
    /// Create a new Wayland application
    fn new(
        registry_state: RegistryState,
        seat_state: SeatState,
        output_state: OutputState,
        shm: Shm,
        layer_shell: LayerShell,
        compositor_state: CompositorState,
        display_ptr: *mut std::ffi::c_void,
        image: ImageData,
        opacity: f32,
        use_gpu: bool,
    ) -> Self {
        let menu_text_metrics = Metrics::new(14.0, 18.0);
        let menu_text_attrs = AttrsOwned::new(Attrs::new().family(Family::Name("Noto Sans")));

        Self {
            registry_state,
            seat_state,
            output_state,
            shm,
            layer_shell,
            compositor_state,
            display_ptr,
            original_aspect_ratio: image.width as f32 / image.height as f32,
            image,
            opacity,
            should_exit: false,
            display_width: 1920,
            display_height: 1080,
            layer_surface: None,
            pool: None,
            buffer: None,
            width: 0,
            height: 0,
            configured: false,
            margin_left: 100,
            margin_top: 100,
            pointer_pos: (0.0, 0.0),
            themed_pointer: None,
            set_cursor_on_next_frame: None,
            dragging: false,
            drag_start_pos: (0.0, 0.0),
            drag_start_margin: (0, 0),
            resizing: false,
            resize_edge: ResizeEdge::None,
            resize_start_pos: (0.0, 0.0),
            resize_start_size: (0, 0),
            resize_start_margin: (0, 0),
            last_click_time: None,
            last_click_pos: (0.0, 0.0),
            menu_state: MenuState::Hidden,
            menu_pos: (0, 0),
            menu_hover_item: None,
            needs_redraw: false,
            scale_mode: ScaleMode::KeepAspectRatio,
            cached_scaled_image: None,
            cached_scaled_size: (0, 0),
            last_resize_draw: None,
            use_gpu,
            gpu_renderer: None,
            gpu_initialized: false,
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
            menu_text_attrs,
            menu_text_metrics,
        }
    }

    /// Detect which resize edge the pointer is near
    fn detect_resize_edge(&self, x: f64, y: f64) -> ResizeEdge {
        let w = self.width as f64;
        let h = self.height as f64;

        let near_left = x < RESIZE_MARGIN;
        let near_right = x > w - RESIZE_MARGIN;
        let near_top = y < RESIZE_MARGIN;
        let near_bottom = y > h - RESIZE_MARGIN;

        match (near_left, near_right, near_top, near_bottom) {
            (true, false, true, false) => ResizeEdge::TopLeft,
            (false, true, true, false) => ResizeEdge::TopRight,
            (true, false, false, true) => ResizeEdge::BottomLeft,
            (false, true, false, true) => ResizeEdge::BottomRight,
            (true, false, false, false) => ResizeEdge::Left,
            (false, true, false, false) => ResizeEdge::Right,
            (false, false, true, false) => ResizeEdge::Top,
            (false, false, false, true) => ResizeEdge::Bottom,
            _ => ResizeEdge::None,
        }
    }

    /// Check if a point is within the menu
    fn get_menu_item_at(&self, x: f64, y: f64) -> Option<usize> {
        if self.menu_state != MenuState::Visible {
            return None;
        }

        let menu_x = self.menu_pos.0 as f64;
        let menu_y = self.menu_pos.1 as f64;
        let menu_w = MENU_WIDTH as f64;
        let menu_items = self.get_menu_items();
        let menu_h = (menu_items.len() * MENU_ITEM_HEIGHT as usize) as f64;

        if x >= menu_x && x < menu_x + menu_w && y >= menu_y && y < menu_y + menu_h {
            let item_idx = ((y - menu_y) / MENU_ITEM_HEIGHT as f64) as usize;
            if item_idx < menu_items.len() {
                return Some(item_idx);
            }
        }
        None
    }

    /// Get dynamic menu items based on current state
    fn get_menu_items(&self) -> Vec<&'static str> {
        let scale_mode_text = match self.scale_mode {
            ScaleMode::KeepAspectRatio => "📐 Scale: Free",
            ScaleMode::FreeScale => "📐 Scale: Keep Ratio",
        };
        vec![
            "❌ Close",
            "📋 Copy to Clipboard",
            "🔆 Opacity +",
            "🔅 Opacity -",
            scale_mode_text,
        ]
    }

    /// Handle menu item selection
    fn handle_menu_action(&mut self, item: usize) {
        match item {
            MENU_ITEM_CLOSE => {
                info!("Menu: Close selected");
                self.should_exit = true;
            }
            MENU_ITEM_COPY => {
                info!("Menu: Copy to clipboard selected");
                self.copy_to_clipboard();
            }
            MENU_ITEM_OPACITY_UP => {
                self.adjust_opacity(OPACITY_STEP);
            }
            MENU_ITEM_OPACITY_DOWN => {
                self.adjust_opacity(-OPACITY_STEP);
            }
            MENU_ITEM_SCALE_MODE => {
                self.toggle_scale_mode();
            }
            _ => {}
        }
        self.menu_state = MenuState::Hidden;
        self.needs_redraw = true;
    }

    /// Toggle scale mode between keep aspect ratio and free scale
    fn toggle_scale_mode(&mut self) {
        self.scale_mode = match self.scale_mode {
            ScaleMode::KeepAspectRatio => {
                info!("Scale mode: Free scale");
                ScaleMode::FreeScale
            }
            ScaleMode::FreeScale => {
                info!("Scale mode: Keep aspect ratio");
                ScaleMode::KeepAspectRatio
            }
        };
        // Invalidate cache when mode changes
        self.cached_scaled_image = None;
    }

    /// Adjust opacity by delta
    fn adjust_opacity(&mut self, delta: f32) {
        let new_opacity = (self.opacity + delta).clamp(0.1, 1.0);
        if (new_opacity - self.opacity).abs() > f32::EPSILON {
            self.opacity = new_opacity;
            info!("Opacity adjusted to: {:.2}", self.opacity);
            self.needs_redraw = true;
        }
    }

    /// Copy image to clipboard using wl-copy or xclip
    fn copy_to_clipboard(&self) {
        // Create a temporary PNG file
        let temp_path = "/tmp/rspin_clipboard.png";

        // Convert BGRA back to RGBA for saving
        let mut rgba_data = self.image.rgba_data.clone();
        for pixel in rgba_data.chunks_exact_mut(4) {
            pixel.swap(0, 2); // Swap B and R back
        }

        // Save as PNG
        if let Err(e) = image::save_buffer(
            temp_path,
            &rgba_data,
            self.image.width,
            self.image.height,
            image::ColorType::Rgba8,
        ) {
            error!("Failed to save temp image: {}", e);
            return;
        }

        // Try wl-copy first (Wayland native)
        let result = Command::new("wl-copy")
            .arg("--type")
            .arg("image/png")
            .arg("-f")
            .arg(temp_path)
            .spawn();

        match result {
            Ok(mut child) => {
                let _ = child.wait();
                info!("Image copied to clipboard via wl-copy");
            }
            Err(_) => {
                // Fallback to xclip
                let result = Command::new("xclip")
                    .arg("-selection")
                    .arg("clipboard")
                    .arg("-t")
                    .arg("image/png")
                    .arg("-i")
                    .arg(temp_path)
                    .spawn();

                match result {
                    Ok(mut child) => {
                        let _ = child.wait();
                        info!("Image copied to clipboard via xclip");
                    }
                    Err(e) => {
                        error!("Failed to copy to clipboard: {}. Install wl-copy or xclip.", e);
                    }
                }
            }
        }

        // Clean up temp file
        let _ = std::fs::remove_file(temp_path);
    }

    /// Update window position using layer shell margins
    fn update_position(&mut self) {
        if let Some(ref layer_surface) = self.layer_surface {
            layer_surface.set_anchor(Anchor::TOP | Anchor::LEFT);
            layer_surface.set_margin(self.margin_top, 0, 0, self.margin_left);
            layer_surface.commit();
        }
    }

    /// Update window size with optional frame rate limiting
    fn update_size(&mut self) {
        // Frame rate limiting during resize (target ~30fps = 33ms between frames)
        const MIN_FRAME_INTERVAL_MS: u128 = 25;

        let max_width = self.display_width.max(MIN_SIZE).min(MAX_SIZE);
        let max_height = self.display_height.max(MIN_SIZE).min(MAX_SIZE);
        self.width = self.width.clamp(MIN_SIZE, max_width);
        self.height = self.height.clamp(MIN_SIZE, max_height);
        
        if self.resizing {
            if let Some(last_draw) = self.last_resize_draw {
                let elapsed = last_draw.elapsed().as_millis();
                if elapsed < MIN_FRAME_INTERVAL_MS {
                    // Skip this frame, just update layer shell size
                    if let Some(ref layer_surface) = self.layer_surface {
                        layer_surface.set_size(self.width, self.height);
                        layer_surface.commit();
                    }
                    return;
                }
            }
            self.last_resize_draw = Some(Instant::now());
        }

        if let Some(ref layer_surface) = self.layer_surface {
            layer_surface.set_size(self.width, self.height);
            layer_surface.commit();
        }
        // Reset pool to force buffer recreation
        self.pool = None;
        self.needs_redraw = true;
    }

    /// Initialize GPU renderer from Wayland surface
    fn init_gpu_renderer(&mut self) {
        if self.gpu_initialized {
            return;
        }

        let layer_surface = match &self.layer_surface {
            Some(ls) => ls,
            None => {
                warn!("Cannot init GPU: no layer surface");
                return;
            }
        };

        // Get raw pointers from Wayland objects
        // The Proxy trait provides id() which gives ObjectId
        // With wayland-backend client_system feature, ObjectId.as_ptr() is available
        let wl_surface = layer_surface.wl_surface();
        let surface_ptr = wl_surface.id().as_ptr() as *mut std::ffi::c_void;

        // Use the display pointer we stored
        let display_ptr = self.display_ptr;

        if display_ptr.is_null() {
            warn!("Display pointer is null, falling back to CPU rendering");
            self.use_gpu = false;
            return;
        }

        info!("Initializing GPU renderer...");
        info!("  Surface ptr: {:?}", surface_ptr);
        info!("  Display ptr: {:?}", display_ptr);
        info!("  Size: {}x{}", self.width, self.height);

        match WgpuRenderer::new(display_ptr, surface_ptr, self.width, self.height) {
            Ok(mut renderer) => {
                // Upload initial texture
                if let Err(e) = renderer.upload_texture(&self.image) {
                    warn!("Failed to upload texture to GPU: {:?}", e);
                    self.use_gpu = false;
                    return;
                }
                renderer.update_opacity(self.opacity);
                self.gpu_renderer = Some(renderer);
                self.gpu_initialized = true;
                info!("GPU renderer initialized successfully");
            }
            Err(e) => {
                warn!("Failed to initialize GPU renderer: {:?}", e);
                warn!("Falling back to CPU rendering");
                self.use_gpu = false;
            }
        }
    }

    /// Draw the image to the surface buffer with scaling support
    fn draw(&mut self, _qh: &QueueHandle<Self>) {
        if !self.configured {
            return;
        }

        if self.layer_surface.is_none() {
            return;
        }

        // Clamp size to display bounds to avoid oversized buffers
        let max_width = self.display_width.max(MIN_SIZE).min(MAX_SIZE);
        let max_height = self.display_height.max(MIN_SIZE).min(MAX_SIZE);
        self.width = self.width.clamp(MIN_SIZE, max_width);
        self.height = self.height.clamp(MIN_SIZE, max_height);

        let menu_pos = self.menu_pos;
        let menu_hover = self.menu_hover_item;
        let menu_items = if self.menu_state == MenuState::Visible {
            Some(self.get_menu_items())
        } else {
            None
        };

        // Try GPU rendering first if enabled
        if self.use_gpu && self.gpu_renderer.is_some() {
            if let Some(ref items) = menu_items {
                self.update_gpu_menu_overlay(menu_pos, menu_hover, items);
            } else if let Some(renderer) = self.gpu_renderer.as_mut() {
                renderer.clear_overlay_texture();
            }

            if self.draw_gpu() {
                return;
            }
            // Fall back to CPU rendering if GPU fails
            warn!("GPU rendering failed, falling back to CPU");
        }

        // CPU rendering path
        self.draw_cpu();
    }

    /// Draw using GPU (wgpu)
    fn draw_gpu(&mut self) -> bool {
        let renderer = match self.gpu_renderer.as_mut() {
            Some(r) => r,
            None => return false,
        };

        // Handle resize
        renderer.resize(self.width, self.height);

        // Update opacity
        renderer.update_opacity(self.opacity);

        // Render
        match renderer.render() {
            Ok(true) => {
                // Commit the surface to show the frame
                if let Some(ref layer_surface) = self.layer_surface {
                    layer_surface.wl_surface().commit();
                }
                self.needs_redraw = false;
                true
            }
            Ok(false) => {
                // No texture or skipped frame
                false
            }
            Err(e) => {
                warn!("GPU render error: {:?}", e);
                false
            }
        }
    }

    fn update_gpu_menu_overlay(
        &mut self,
        menu_pos: (i32, i32),
        menu_hover_item: Option<usize>,
        menu_items: &[&str],
    ) {
        let surface_width = self.width;
        let surface_height = self.height;
        if surface_width == 0 || surface_height == 0 {
            return;
        }

        let menu_x = menu_pos.0.max(0).min(surface_width as i32 - 1).max(0);
        let menu_y = menu_pos.1.max(0).min(surface_height as i32 - 1).max(0);

        let menu_width = MENU_WIDTH.min(surface_width.saturating_sub(menu_x as u32));
        let menu_height =
            (menu_items.len() as u32 * MENU_ITEM_HEIGHT).min(surface_height.saturating_sub(menu_y as u32));

        if menu_width == 0 || menu_height == 0 {
            if let Some(renderer) = self.gpu_renderer.as_mut() {
                renderer.clear_overlay_texture();
            }
            return;
        }

        let mut buffer = vec![0u8; (menu_width * menu_height * 4) as usize];
        self.render_menu_overlay_contents(&mut buffer, menu_width, menu_height, menu_hover_item, menu_items);
        for pixel in buffer.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }

        let viewport = [
            menu_x as f32,
            menu_y as f32,
            menu_width as f32,
            menu_height as f32,
        ];

        if let Some(renderer) = self.gpu_renderer.as_mut() {
            if let Err(e) = renderer.update_overlay_texture(menu_width, menu_height, viewport, &buffer) {
                warn!("Failed to upload menu overlay: {:?}", e);
            }
        }
    }

    /// Draw using CPU (shared memory buffer)
    fn draw_cpu(&mut self) {
        // Clamp window size to prevent buffer allocation failures
        self.width = self.width.clamp(MIN_SIZE, MAX_SIZE);
        self.height = self.height.clamp(MIN_SIZE, MAX_SIZE);

        let width = self.width;
        let height = self.height;

        // Calculate buffer size (4 bytes per pixel for ARGB)
        let stride = width as i32 * 4;
        let buffer_size = (stride * height as i32) as usize;

        // Check if buffer size is reasonable
        if buffer_size > MAX_BUFFER_SIZE {
            error!("Buffer size too large: {} bytes, max: {} bytes", buffer_size, MAX_BUFFER_SIZE);
            // Scale down to fit
            let scale = (MAX_BUFFER_SIZE as f32 / buffer_size as f32).sqrt();
            self.width = (width as f32 * scale) as u32;
            self.height = (height as f32 * scale) as u32;
            return; // Will redraw on next frame with new size
        }

        // Gather state needed for rendering before mutable borrow
        let is_resizing = self.resizing;
        let opacity = self.opacity;
        let menu_visible = self.menu_state == MenuState::Visible;
        let menu_pos = self.menu_pos;
        let menu_hover = self.menu_hover_item;
        let menu_items: Vec<&'static str> = if menu_visible {
            self.get_menu_items()
        } else {
            vec![]
        };

        // Initialize pool if needed
        if self.pool.is_none() {
            match SlotPool::new(buffer_size, &self.shm) {
                Ok(pool) => self.pool = Some(pool),
                Err(e) => {
                    error!(
                        "Failed to create slot pool: {}. Buffer size: {} bytes",
                        e, buffer_size
                    );
                    return;
                }
            }
        }

        // Temporarily take ownership of the pool to avoid borrow conflicts during rendering
        let mut pool = match self.pool.take() {
            Some(pool) => pool,
            None => return,
        };

        // Resize pool if needed
        if pool.len() < buffer_size {
            if let Err(e) = pool.resize(buffer_size) {
                error!("Failed to resize pool to {} bytes: {}", buffer_size, e);
                // Drop pool so a new one will be created next frame
                self.pool = None;
                return;
            }
        }

        // Create buffer
        let (buffer, canvas) =
            match pool.create_buffer(width as i32, height as i32, stride, wl_shm::Format::Argb8888)
            {
                Ok(buf) => buf,
                Err(e) => {
                    error!("Failed to create buffer {}x{}: {}", width, height, e);
                    self.pool = Some(pool);
                    return;
                }
            };

        let cache_enabled = !self.use_gpu;

        // Choose rendering method based on whether we're resizing
        if is_resizing {
            // Use fast nearest-neighbor during resize for responsiveness
            Self::render_image_fast(&self.image, canvas, width, height, opacity);
        } else if cache_enabled {
            // Use high-quality bilinear interpolation when not resizing
            // Check if we can use cached image
            if self.cached_scaled_size == (width, height) {
                if let Some(ref cached) = self.cached_scaled_image {
                    // Apply opacity to cached image
                    Self::apply_opacity_to_canvas(cached, canvas, opacity);
                } else {
                    Self::render_image_static(&self.image, canvas, width, height, opacity);
                }
            } else {
                Self::render_image_static(&self.image, canvas, width, height, opacity);
                // Cache the scaled image (without opacity applied)
                let mut cached = vec![0u8; buffer_size];
                Self::render_image_static(&self.image, &mut cached, width, height, 1.0);
                self.cached_scaled_image = Some(cached);
                self.cached_scaled_size = (width, height);
            }
        } else {
            Self::render_image_static(&self.image, canvas, width, height, opacity);
            self.cached_scaled_image = None;
            self.cached_scaled_size = (0, 0);
        }

        // Draw context menu if visible
        if menu_visible {
            self.render_menu(canvas, width, height, menu_pos, menu_hover, &menu_items);
        }

        // Draw resize handles (subtle border)
        Self::render_resize_border_static(canvas, width, height);

        // Attach and commit
        let layer_surface = self.layer_surface.as_ref().unwrap();
        let surface = layer_surface.wl_surface();
        buffer.attach_to(surface).expect("Failed to attach buffer");
        surface.damage_buffer(0, 0, width as i32, height as i32);
        surface.commit();

        self.pool = Some(pool);
        self.buffer = Some(buffer);
        self.needs_redraw = false;
    }

    /// Render the image to the canvas (static version to avoid borrow issues)
    fn render_image_static(image: &ImageData, canvas: &mut [u8], width: u32, height: u32, opacity: f32) {
        // Choose best mipmap level for quality rendering
        let scale_ratio = (width as f32 / image.width as f32).min(height as f32 / image.height as f32);
        
        let (img_width, img_height, src_data) = if scale_ratio < 0.7 && !image.mipmaps.is_empty() {
            // Find the best mipmap level (choose one slightly larger than needed)
            let mut best_level = 0;
            for (i, mipmap) in image.mipmaps.iter().enumerate() {
                let mip_scale = mipmap.width as f32 / image.width as f32;
                if mip_scale >= scale_ratio {
                    best_level = i.saturating_sub(1); // Use previous level for better quality
                    break;
                }
                best_level = i;
            }
            
            if best_level >= image.mipmaps.len() {
                best_level = image.mipmaps.len() - 1;
            }
            
            if best_level > 0 && best_level <= image.mipmaps.len() {
                let mipmap = &image.mipmaps[best_level - 1];
                (mipmap.width, mipmap.height, &mipmap.data[..])
            } else {
                (image.width, image.height, &image.rgba_data[..])
            }
        } else {
            (image.width, image.height, &image.rgba_data[..])
        };

        // Fill with transparent background first
        for pixel in canvas.chunks_exact_mut(4) {
            pixel[0] = 0; // B
            pixel[1] = 0; // G
            pixel[2] = 0; // R
            pixel[3] = 0; // A
        }

        // Calculate scale factors for rendering
        let scale_x = img_width as f32 / width as f32;
        let scale_y = img_height as f32 / height as f32;

        // Render with bilinear interpolation for smooth scaling
        for y in 0..height {
            for x in 0..width {
                let src_x = x as f32 * scale_x;
                let src_y = y as f32 * scale_y;

                let x0 = src_x.floor() as u32;
                let y0 = src_y.floor() as u32;
                let x1 = (x0 + 1).min(img_width - 1);
                let y1 = (y0 + 1).min(img_height - 1);

                let fx = src_x - x0 as f32;
                let fy = src_y - y0 as f32;

                let get_pixel = |px: u32, py: u32| -> [u8; 4] {
                    let idx = ((py * img_width + px) * 4) as usize;
                    if idx + 3 < src_data.len() {
                        [
                            src_data[idx],
                            src_data[idx + 1],
                            src_data[idx + 2],
                            src_data[idx + 3],
                        ]
                    } else {
                        [0, 0, 0, 0]
                    }
                };

                let p00 = get_pixel(x0, y0);
                let p10 = get_pixel(x1, y0);
                let p01 = get_pixel(x0, y1);
                let p11 = get_pixel(x1, y1);

                let interpolate = |c: usize| -> u8 {
                    let v00 = p00[c] as f32;
                    let v10 = p10[c] as f32;
                    let v01 = p01[c] as f32;
                    let v11 = p11[c] as f32;

                    let v0 = v00 * (1.0 - fx) + v10 * fx;
                    let v1 = v01 * (1.0 - fx) + v11 * fx;
                    let v = v0 * (1.0 - fy) + v1 * fy;

                    v.round().clamp(0.0, 255.0) as u8
                };

                let dst_idx = ((y * width + x) * 4) as usize;
                if dst_idx + 3 < canvas.len() {
                    let src_alpha = interpolate(3) as f32 / 255.0;
                    let final_alpha = (src_alpha * opacity * 255.0) as u8;

                    canvas[dst_idx] = interpolate(0);
                    canvas[dst_idx + 1] = interpolate(1);
                    canvas[dst_idx + 2] = interpolate(2);
                    canvas[dst_idx + 3] = final_alpha;
                }
            }
        }
    }

    /// Fast nearest-neighbor rendering for responsive resize with mipmap optimization
    fn render_image_fast(image: &ImageData, canvas: &mut [u8], width: u32, height: u32, opacity: f32) {
        // Choose best mipmap level based on target size
        // Use mipmap when downscaling significantly for better performance
        let scale_ratio = (width as f32 / image.width as f32).min(height as f32 / image.height as f32);
        
        let (img_width, img_height, src_data) = if scale_ratio < 0.5 && !image.mipmaps.is_empty() {
            // Find the best mipmap level
            let mut best_level = 0;
            for (i, mipmap) in image.mipmaps.iter().enumerate() {
                let mip_scale = mipmap.width as f32 / image.width as f32;
                if mip_scale >= scale_ratio * 0.75 {
                    best_level = i;
                    break;
                }
                best_level = i;
            }
            
            let mipmap = &image.mipmaps[best_level];
            (mipmap.width, mipmap.height, &mipmap.data[..])
        } else {
            (image.width, image.height, &image.rgba_data[..])
        };

        // Pre-compute scale factors as fixed-point for faster integer math
        let scale_x_fp = ((img_width as u64) << 16) / width as u64;
        let scale_y_fp = ((img_height as u64) << 16) / height as u64;
        let opacity_i = (opacity * 255.0) as u32;
        let img_stride = img_width * 4;

        // Pre-compute X lookup table to avoid repeated calculations per row
        let x_lut: Vec<u32> = (0..width)
            .map(|x| {
                let src_x = ((x as u64 * scale_x_fp) >> 16) as u32;
                src_x.min(img_width - 1)
            })
            .collect();

        // Process each row with SIMD-friendly memory access patterns
        for y in 0..height {
            let src_y = (((y as u64) * scale_y_fp) >> 16) as u32;
            let src_y = src_y.min(img_height - 1);
            let src_row_offset = (src_y * img_stride) as usize;
            let dst_row_offset = (y * width * 4) as usize;

            // Process row with pre-computed X values
            for (x, &src_x) in x_lut.iter().enumerate() {
                let src_idx = src_row_offset + (src_x * 4) as usize;
                let dst_idx = dst_row_offset + x * 4;

                if src_idx + 3 < src_data.len() && dst_idx + 3 < canvas.len() {
                    // Fast alpha blend with integer math
                    let src_alpha = src_data[src_idx + 3] as u32;
                    let final_alpha = ((src_alpha * opacity_i) >> 8) as u8;

                    // Direct copy (compiler can optimize this to vector operations)
                    canvas[dst_idx] = src_data[src_idx];
                    canvas[dst_idx + 1] = src_data[src_idx + 1];
                    canvas[dst_idx + 2] = src_data[src_idx + 2];
                    canvas[dst_idx + 3] = final_alpha;
                }
            }
        }
    }

    /// Apply opacity to cached image data
    fn apply_opacity_to_canvas(cached: &[u8], canvas: &mut [u8], opacity: f32) {
        for (dst, src) in canvas.chunks_exact_mut(4).zip(cached.chunks_exact(4)) {
            let src_alpha = src[3] as f32 / 255.0;
            let final_alpha = (src_alpha * opacity * 255.0) as u8;
            dst[0] = src[0];
            dst[1] = src[1];
            dst[2] = src[2];
            dst[3] = final_alpha;
        }
    }

    /// Render the context menu (static version)
    fn render_menu(&mut self, canvas: &mut [u8], canvas_width: u32, canvas_height: u32, menu_pos: (i32, i32), menu_hover_item: Option<usize>, menu_items: &[&str]) {
        let menu_x = menu_pos.0.max(0) as u32;
        let menu_y = menu_pos.1.max(0) as u32;

        for (i, item) in menu_items.iter().enumerate() {
            let item_y = menu_y + (i as u32 * MENU_ITEM_HEIGHT);
            let is_hovered = menu_hover_item == Some(i);

            // Draw menu item background with rounded appearance
            let bg_color: [u8; 4] = if is_hovered {
                [100, 150, 220, 240] // Highlighted: BGRA blue
            } else {
                [45, 45, 48, 240] // Normal: BGRA dark gray (GTK-like)
            };

            for y in item_y..(item_y + MENU_ITEM_HEIGHT).min(canvas_height) {
                for x in menu_x..(menu_x + MENU_WIDTH).min(canvas_width) {
                    let idx = ((y * canvas_width + x) * 4) as usize;
                    if idx + 3 < canvas.len() {
                        canvas[idx] = bg_color[0];
                        canvas[idx + 1] = bg_color[1];
                        canvas[idx + 2] = bg_color[2];
                        canvas[idx + 3] = bg_color[3];
                    }
                }
            }

            // Draw text using cosmic-text
            let text_x = menu_x + 12;
            let text_y = item_y + 5;
            let text_color = if is_hovered {
                [255, 255, 255, 255] // White when hovered
            } else {
                [220, 220, 220, 255] // Light gray normally
            };
            self.draw_text_cosmic(canvas, canvas_width, canvas_height, text_x, text_y, item, text_color);
        }

        // Draw menu border with shadow effect
        let border_color: [u8; 4] = [80, 80, 80, 255];
        let menu_height = menu_items.len() as u32 * MENU_ITEM_HEIGHT;

        // Top and bottom borders
        for x in menu_x..(menu_x + MENU_WIDTH).min(canvas_width) {
            for &y in &[menu_y, (menu_y + menu_height - 1).min(canvas_height - 1)] {
                let idx = ((y * canvas_width + x) * 4) as usize;
                if idx + 3 < canvas.len() {
                    canvas[idx] = border_color[0];
                    canvas[idx + 1] = border_color[1];
                    canvas[idx + 2] = border_color[2];
                    canvas[idx + 3] = border_color[3];
                }
            }
        }

        // Left and right borders
        for y in menu_y..(menu_y + menu_height).min(canvas_height) {
            for &x in &[menu_x, (menu_x + MENU_WIDTH - 1).min(canvas_width - 1)] {
                let idx = ((y * canvas_width + x) * 4) as usize;
                if idx + 3 < canvas.len() {
                    canvas[idx] = border_color[0];
                    canvas[idx + 1] = border_color[1];
                    canvas[idx + 2] = border_color[2];
                    canvas[idx + 3] = border_color[3];
                }
            }
        }
    }

    /// Render menu contents into a local buffer (used for GPU overlay)
    fn render_menu_overlay_contents(
        &mut self,
        canvas: &mut [u8],
        canvas_width: u32,
        canvas_height: u32,
        menu_hover_item: Option<usize>,
        menu_items: &[&str],
    ) {
        for (i, item) in menu_items.iter().enumerate() {
            let item_y = (i as u32) * MENU_ITEM_HEIGHT;
            if item_y >= canvas_height {
                break;
            }
            let is_hovered = menu_hover_item == Some(i);

            let bg_color: [u8; 4] = if is_hovered {
                [100, 150, 220, 240]
            } else {
                [45, 45, 48, 240]
            };

            for y in item_y..(item_y + MENU_ITEM_HEIGHT).min(canvas_height) {
                for x in 0..canvas_width.min(MENU_WIDTH) {
                    let idx = ((y * canvas_width + x) * 4) as usize;
                    if idx + 3 < canvas.len() {
                        canvas[idx] = bg_color[0];
                        canvas[idx + 1] = bg_color[1];
                        canvas[idx + 2] = bg_color[2];
                        canvas[idx + 3] = bg_color[3];
                    }
                }
            }

            let text_x = 12;
            let text_y = item_y + 5;
            let text_color = if is_hovered {
                [255, 255, 255, 255]
            } else {
                [220, 220, 220, 255]
            };
            self.draw_text_cosmic(canvas, canvas_width, canvas_height, text_x, text_y, item, text_color);
        }

        let border_color: [u8; 4] = [80, 80, 80, 255];
        let menu_height = canvas_height.min(menu_items.len() as u32 * MENU_ITEM_HEIGHT);

        for x in 0..canvas_width.min(MENU_WIDTH) {
            for &y in &[0, menu_height.saturating_sub(1)] {
                let idx = ((y * canvas_width + x) * 4) as usize;
                if idx + 3 < canvas.len() {
                    canvas[idx] = border_color[0];
                    canvas[idx + 1] = border_color[1];
                    canvas[idx + 2] = border_color[2];
                    canvas[idx + 3] = border_color[3];
                }
            }
        }

        for y in 0..menu_height {
            for &x in &[0, canvas_width.min(MENU_WIDTH).saturating_sub(1)] {
                let idx = ((y * canvas_width + x) * 4) as usize;
                if idx + 3 < canvas.len() {
                    canvas[idx] = border_color[0];
                    canvas[idx + 1] = border_color[1];
                    canvas[idx + 2] = border_color[2];
                    canvas[idx + 3] = border_color[3];
                }
            }
        }
    }

    /// Draw text using cosmic-text for proper font rendering
    fn draw_text_cosmic(
        &mut self,
        canvas: &mut [u8],
        canvas_width: u32,
        canvas_height: u32,
        x: u32,
        y: u32,
        text: &str,
        color: [u8; 4],
    ) {
        let mut buffer = Buffer::new(&mut self.font_system, self.menu_text_metrics);
        buffer.set_size(
            &mut self.font_system,
            Some(MENU_WIDTH as f32 - 24.0),
            Some(MENU_ITEM_HEIGHT as f32),
        );
        buffer.set_text(
            &mut self.font_system,
            text,
            self.menu_text_attrs.as_attrs(),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

        let rgba = TextColor::rgba(color[2], color[1], color[0], color[3]);
        let origin_x = x as i32;
        let origin_y = y as i32;

        buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            rgba,
            |px, py, _w, _h, glyph_color| {
                let pixel_x = origin_x + px;
                let pixel_y = origin_y + py;

                if pixel_x < 0
                    || pixel_x >= canvas_width as i32
                    || pixel_y < 0
                    || pixel_y >= canvas_height as i32
                {
                    return;
                }

                let idx = ((pixel_y as u32 * canvas_width + pixel_x as u32) * 4) as usize;
                if idx + 3 >= canvas.len() {
                    return;
                }

                let [r, g, b, a] = glyph_color.as_rgba();
                let src = [b, g, r, a];

                let src_alpha = src[3] as f32 / 255.0;
                if src_alpha <= 0.0 {
                    return;
                }
                let dst_alpha = canvas[idx + 3] as f32 / 255.0;
                let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);
                if out_alpha <= 0.0 {
                    return;
                }

                let blend = |src_channel: u8, dst_channel: u8| -> u8 {
                    ((src_channel as f32 * src_alpha
                        + dst_channel as f32 * dst_alpha * (1.0 - src_alpha))
                        / out_alpha)
                        .round()
                        .clamp(0.0, 255.0) as u8
                };

                canvas[idx] = blend(src[0], canvas[idx]);
                canvas[idx + 1] = blend(src[1], canvas[idx + 1]);
                canvas[idx + 2] = blend(src[2], canvas[idx + 2]);
                canvas[idx + 3] = (out_alpha * 255.0) as u8;
            },
        );
    }

    /// Render resize border indicator (static version)
    fn render_resize_border_static(canvas: &mut [u8], width: u32, height: u32) {
        let border_color: [u8; 4] = [150, 150, 150, 100];

        // Draw subtle corner indicators
        let corner_size = RESIZE_MARGIN as u32;

        // Draw corner indicators
        for i in 0..corner_size {
            // Top-left
            Self::draw_pixel(canvas, width, height, i, 0, border_color);
            Self::draw_pixel(canvas, width, height, 0, i, border_color);
            // Top-right
            Self::draw_pixel(canvas, width, height, width.saturating_sub(1).saturating_sub(i), 0, border_color);
            Self::draw_pixel(canvas, width, height, width.saturating_sub(1), i, border_color);
            // Bottom-left
            Self::draw_pixel(canvas, width, height, i, height.saturating_sub(1), border_color);
            Self::draw_pixel(canvas, width, height, 0, height.saturating_sub(1).saturating_sub(i), border_color);
            // Bottom-right
            Self::draw_pixel(canvas, width, height, width.saturating_sub(1).saturating_sub(i), height.saturating_sub(1), border_color);
            Self::draw_pixel(canvas, width, height, width.saturating_sub(1), height.saturating_sub(1).saturating_sub(i), border_color);
        }
    }

    /// Helper to draw a single pixel
    fn draw_pixel(canvas: &mut [u8], canvas_width: u32, canvas_height: u32, x: u32, y: u32, color: [u8; 4]) {
        if x < canvas_width && y < canvas_height {
            let idx = ((y * canvas_width + x) * 4) as usize;
            if idx + 3 < canvas.len() {
                canvas[idx] = color[0];
                canvas[idx + 1] = color[1];
                canvas[idx + 2] = color[2];
                canvas[idx + 3] = color[3];
            }
        }
    }
}

// Implement required traits for smithay-client-toolkit

impl CompositorHandler for WaylandApp {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
        debug!("Scale factor changed");
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
        debug!("Transform changed");
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        if self.needs_redraw {
            self.draw(qh);
        }
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for WaylandApp {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        debug!("New output detected");
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        debug!("Output updated");
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        debug!("Output destroyed");
    }
}

impl LayerShellHandler for WaylandApp {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        info!("Layer surface closed");
        self.should_exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        debug!("Layer surface configured: {:?}", configure);

        // When dragging or resizing, ignore compositor's size suggestions
        // to allow the window to extend beyond screen boundaries
        if !self.dragging && !self.resizing {
            // Only accept compositor's size if we're not actively manipulating the window
            if configure.new_size.0 > 0 && configure.new_size.0 != self.width {
                self.width = configure.new_size.0;
            }
            if configure.new_size.1 > 0 && configure.new_size.1 != self.height {
                self.height = configure.new_size.1;
            }
        }
        // If dragging/resizing, keep our own size and re-request it
        else if let Some(ref layer_surface) = self.layer_surface {
            layer_surface.set_size(self.width, self.height);
            layer_surface.commit();
        }

        self.configured = true;
        self.needs_redraw = true;

        // Initialize GPU renderer if requested and not yet initialized
        if self.use_gpu && !self.gpu_initialized {
            self.init_gpu_renderer();
        }

        // Draw initial frame
        self.draw(qh);
    }
}

impl SeatHandler for WaylandApp {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {
        debug!("New seat");
    }

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        debug!("New capability: {:?}", capability);

        if capability == Capability::Keyboard {
            if let Err(e) = self.seat_state.get_keyboard(qh, &seat, None) {
                error!("Failed to get keyboard: {}", e);
            }
        }
        if capability == Capability::Pointer && self.themed_pointer.is_none() {
            debug!("Creating themed pointer");
            let surface = self.compositor_state.create_surface(qh);
            match self.seat_state.get_pointer_with_theme(
                qh,
                &seat,
                self.shm.wl_shm(),
                surface,
                ThemeSpec::default(),
            ) {
                Ok(pointer) => {
                    self.themed_pointer = Some(pointer);
                    debug!("Themed pointer created successfully");
                }
                Err(e) => error!("Failed to create themed pointer: {}", e),
            }
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        _capability: Capability,
    ) {
        debug!("Capability removed");
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {
        debug!("Seat removed");
    }
}

impl KeyboardHandler for WaylandApp {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
        debug!("Keyboard entered surface");
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
        debug!("Keyboard left surface");
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        debug!("Key pressed: {:?}", event.keysym);

        // Close on Escape or Q key
        if event.keysym == Keysym::Escape || event.keysym == Keysym::q {
            info!("Exit key pressed");
            self.should_exit = true;
        }
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _modifiers: Modifiers,
        _layout: u32,
    ) {
    }
}

impl PointerHandler for WaylandApp {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    debug!("Pointer entered");
                    self.set_cursor_on_next_frame = Some(CursorIcon::Default);
                }
                PointerEventKind::Leave { .. } => {
                    debug!("Pointer left");
                    self.dragging = false;
                    self.resizing = false;
                }
                PointerEventKind::Motion { .. } => {
                    let (x, y) = event.position;
                    self.pointer_pos = (x, y);

                    // Update menu hover state
                    if self.menu_state == MenuState::Visible {
                        let prev_hover = self.menu_hover_item;
                        self.menu_hover_item = self.get_menu_item_at(x, y);
                        if prev_hover != self.menu_hover_item {
                            self.needs_redraw = true;
                        }
                        // Set default cursor when over menu
                        self.set_cursor_on_next_frame = Some(CursorIcon::Default);
                    } else if !self.dragging && !self.resizing {
                        // Update cursor based on resize edge detection
                        let edge = self.detect_resize_edge(x, y);
                        let cursor_icon = match edge {
                            ResizeEdge::Top | ResizeEdge::Bottom => CursorIcon::NsResize,
                            ResizeEdge::Left | ResizeEdge::Right => CursorIcon::EwResize,
                            ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorIcon::NwseResize,
                            ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorIcon::NeswResize,
                            ResizeEdge::None => CursorIcon::Default,
                        };
                        self.set_cursor_on_next_frame = Some(cursor_icon);
                    }

                    // Handle dragging (window move)
                    if self.dragging {
                        let dx = x - self.drag_start_pos.0;
                        let dy = y - self.drag_start_pos.1;

                        // Allow window to go beyond screen boundaries
                        self.margin_left = self.drag_start_margin.0 + dx as i32;
                        self.margin_top = self.drag_start_margin.1 + dy as i32;

                        self.update_position();
                    }

                    // Handle resizing
                    if self.resizing {
                        let dx = (x - self.resize_start_pos.0) as i32;
                        let dy = (y - self.resize_start_pos.1) as i32;

                        let (start_w, start_h) = self.resize_start_size;
                        let (start_ml, start_mt) = self.resize_start_margin;
                        let aspect_ratio = self.original_aspect_ratio;
                        let keep_ratio = self.scale_mode == ScaleMode::KeepAspectRatio;

                        // Calculate new dimensions based on resize edge
                        let (mut new_w, mut new_h, mut new_ml, mut new_mt) = 
                            (start_w, start_h, start_ml, start_mt);

                        match self.resize_edge {
                            ResizeEdge::Right => {
                                new_w = (start_w as i32 + dx).max(MIN_SIZE as i32) as u32;
                                if keep_ratio {
                                    new_h = (new_w as f32 / aspect_ratio) as u32;
                                }
                            }
                            ResizeEdge::Bottom => {
                                new_h = (start_h as i32 + dy).max(MIN_SIZE as i32) as u32;
                                if keep_ratio {
                                    new_w = (new_h as f32 * aspect_ratio) as u32;
                                }
                            }
                            ResizeEdge::BottomRight => {
                                if keep_ratio {
                                    // Use the larger delta to determine scale
                                    let scale_by_x = (start_w as i32 + dx) as f32 / start_w as f32;
                                    let scale_by_y = (start_h as i32 + dy) as f32 / start_h as f32;
                                    let scale = scale_by_x.max(scale_by_y).max(MIN_SIZE as f32 / start_w as f32);
                                    new_w = (start_w as f32 * scale) as u32;
                                    new_h = (start_h as f32 * scale) as u32;
                                } else {
                                    new_w = (start_w as i32 + dx).max(MIN_SIZE as i32) as u32;
                                    new_h = (start_h as i32 + dy).max(MIN_SIZE as i32) as u32;
                                }
                            }
                            ResizeEdge::Left => {
                                let raw_w = (start_w as i32 - dx).max(MIN_SIZE as i32) as u32;
                                if keep_ratio {
                                    new_w = raw_w;
                                    new_h = (new_w as f32 / aspect_ratio) as u32;
                                    let height_diff = new_h as i32 - start_h as i32;
                                    new_mt = start_mt - height_diff / 2;
                                } else {
                                    new_w = raw_w;
                                }
                                new_ml = start_ml + (start_w as i32 - new_w as i32);
                            }
                            ResizeEdge::Top => {
                                let raw_h = (start_h as i32 - dy).max(MIN_SIZE as i32) as u32;
                                if keep_ratio {
                                    new_h = raw_h;
                                    new_w = (new_h as f32 * aspect_ratio) as u32;
                                    let width_diff = new_w as i32 - start_w as i32;
                                    new_ml = start_ml - width_diff / 2;
                                } else {
                                    new_h = raw_h;
                                }
                                new_mt = start_mt + (start_h as i32 - new_h as i32);
                            }
                            ResizeEdge::TopLeft => {
                                if keep_ratio {
                                    let scale_by_x = (start_w as i32 - dx) as f32 / start_w as f32;
                                    let scale_by_y = (start_h as i32 - dy) as f32 / start_h as f32;
                                    let scale = scale_by_x.max(scale_by_y).max(MIN_SIZE as f32 / start_w as f32);
                                    new_w = (start_w as f32 * scale) as u32;
                                    new_h = (start_h as f32 * scale) as u32;
                                } else {
                                    new_w = (start_w as i32 - dx).max(MIN_SIZE as i32) as u32;
                                    new_h = (start_h as i32 - dy).max(MIN_SIZE as i32) as u32;
                                }
                                new_ml = start_ml + (start_w as i32 - new_w as i32);
                                new_mt = start_mt + (start_h as i32 - new_h as i32);
                            }
                            ResizeEdge::TopRight => {
                                if keep_ratio {
                                    let scale_by_x = (start_w as i32 + dx) as f32 / start_w as f32;
                                    let scale_by_y = (start_h as i32 - dy) as f32 / start_h as f32;
                                    let scale = scale_by_x.max(scale_by_y).max(MIN_SIZE as f32 / start_w as f32);
                                    new_w = (start_w as f32 * scale) as u32;
                                    new_h = (start_h as f32 * scale) as u32;
                                } else {
                                    new_w = (start_w as i32 + dx).max(MIN_SIZE as i32) as u32;
                                    new_h = (start_h as i32 - dy).max(MIN_SIZE as i32) as u32;
                                }
                                new_mt = start_mt + (start_h as i32 - new_h as i32);
                            }
                            ResizeEdge::BottomLeft => {
                                if keep_ratio {
                                    let scale_by_x = (start_w as i32 - dx) as f32 / start_w as f32;
                                    let scale_by_y = (start_h as i32 + dy) as f32 / start_h as f32;
                                    let scale = scale_by_x.max(scale_by_y).max(MIN_SIZE as f32 / start_w as f32);
                                    new_w = (start_w as f32 * scale) as u32;
                                    new_h = (start_h as f32 * scale) as u32;
                                } else {
                                    new_w = (start_w as i32 - dx).max(MIN_SIZE as i32) as u32;
                                    new_h = (start_h as i32 + dy).max(MIN_SIZE as i32) as u32;
                                }
                                new_ml = start_ml + (start_w as i32 - new_w as i32);
                            }
                            ResizeEdge::None => {}
                        }

                        // Apply size constraints (min and max)
                        new_w = new_w.clamp(MIN_SIZE, MAX_SIZE);
                        new_h = new_h.clamp(MIN_SIZE, MAX_SIZE);

                        // Check if resulting buffer would be too large
                        let potential_buffer_size = (new_w * new_h * 4) as usize;
                        if potential_buffer_size > MAX_BUFFER_SIZE {
                            // Scale down proportionally
                            let scale = (MAX_BUFFER_SIZE as f32 / potential_buffer_size as f32).sqrt();
                            new_w = (new_w as f32 * scale) as u32;
                            new_h = (new_h as f32 * scale) as u32;
                            info!("Window size capped to {}x{} to prevent buffer overflow", new_w, new_h);
                        }

                        self.width = new_w;
                        self.height = new_h;
                        self.margin_left = new_ml;
                        self.margin_top = new_mt;

                        self.update_position();
                        self.update_size();
                    }
                }
                PointerEventKind::Press { button, .. } => {
                    debug!("Pointer button pressed: {}", button);
                    let (x, y) = self.pointer_pos;

                    if button == BTN_LEFT {
                        // Check if clicking on menu
                        if self.menu_state == MenuState::Visible {
                            if let Some(item) = self.get_menu_item_at(x, y) {
                                self.handle_menu_action(item);
                                self.draw(qh);
                                continue;
                            } else {
                                // Close menu if clicking outside
                                self.menu_state = MenuState::Hidden;
                                self.needs_redraw = true;
                                self.draw(qh);
                            }
                        }

                        // Check for double-click
                        let now = Instant::now();
                        let is_double_click = if let Some(last_time) = self.last_click_time {
                            let elapsed = now.duration_since(last_time).as_millis();
                            let dist = ((x - self.last_click_pos.0).powi(2)
                                + (y - self.last_click_pos.1).powi(2))
                            .sqrt();
                            elapsed < DOUBLE_CLICK_THRESHOLD_MS && dist < 10.0
                        } else {
                            false
                        };

                        if is_double_click {
                            info!("Double-click detected, exiting");
                            self.should_exit = true;
                            continue;
                        }

                        self.last_click_time = Some(now);
                        self.last_click_pos = (x, y);

                        // Check if on resize edge
                        let edge = self.detect_resize_edge(x, y);
                        if edge != ResizeEdge::None {
                            self.resizing = true;
                            self.resize_edge = edge;
                            self.resize_start_pos = (x, y);
                            self.resize_start_size = (self.width, self.height);
                            self.resize_start_margin = (self.margin_left, self.margin_top);
                        } else {
                            // Start dragging for window move
                            self.dragging = true;
                            self.drag_start_pos = (x, y);
                            self.drag_start_margin = (self.margin_left, self.margin_top);
                        }
                    } else if button == BTN_RIGHT {
                        // Show context menu
                        self.menu_state = MenuState::Visible;
                        self.menu_pos = (x as i32, y as i32);

                        // Adjust menu position to stay within window bounds
                        let menu_items = self.get_menu_items();
                        let menu_height = menu_items.len() as i32 * MENU_ITEM_HEIGHT as i32;
                        if self.menu_pos.0 + MENU_WIDTH as i32 > self.width as i32 {
                            self.menu_pos.0 = self.width as i32 - MENU_WIDTH as i32;
                        }
                        if self.menu_pos.1 + menu_height > self.height as i32 {
                            self.menu_pos.1 = self.height as i32 - menu_height;
                        }
                        self.menu_pos.0 = self.menu_pos.0.max(0);
                        self.menu_pos.1 = self.menu_pos.1.max(0);

                        self.needs_redraw = true;
                        self.draw(qh);
                    }
                }
                PointerEventKind::Release { button, .. } => {
                    if button == BTN_LEFT {
                        // If we were resizing, trigger high quality redraw
                        let was_resizing = self.resizing;
                        
                        self.dragging = false;
                        self.resizing = false;
                        self.resize_edge = ResizeEdge::None;
                        
                        if was_resizing {
                            // Invalidate cache to force high-quality render
                            self.cached_scaled_image = None;
                            self.needs_redraw = true;
                            self.draw(qh);
                        }
                    }
                }
                PointerEventKind::Axis {
                    vertical,
                    ..
                } => {
                    // Scroll wheel to adjust opacity
                    if vertical.absolute != 0.0 {
                        let delta = if vertical.absolute > 0.0 {
                            -OPACITY_STEP
                        } else {
                            OPACITY_STEP
                        };
                        self.adjust_opacity(delta);
                        self.draw(qh);
                    }
                }
            }
        }

        // Update cursor at the end of the frame
        if let Some(cursor_icon) = self.set_cursor_on_next_frame.take() {
            if let Some(themed_pointer) = &self.themed_pointer {
                let _ = themed_pointer.set_cursor(_conn, cursor_icon);
            }
        }
    }
}

impl ShmHandler for WaylandApp {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for WaylandApp {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState, SeatState];
}

// Delegate macros
delegate_compositor!(WaylandApp);
delegate_output!(WaylandApp);
delegate_layer!(WaylandApp);
delegate_seat!(WaylandApp);
delegate_keyboard!(WaylandApp);
delegate_pointer!(WaylandApp);
delegate_shm!(WaylandApp);
delegate_registry!(WaylandApp);

/// Run the Wayland application
pub fn run(image: ImageData, opacity: f32, use_gpu: bool) -> Result<()> {
    info!("Connecting to Wayland display");

    // Connect to Wayland display
    let conn = Connection::connect_to_env().context("Failed to connect to Wayland display")?;

    // Initialize registry and event queue
    let (globals, mut event_queue) =
        registry_queue_init(&conn).context("Failed to initialize registry")?;
    let qh = event_queue.handle();

    // Initialize required globals
    let compositor_state =
        CompositorState::bind(&globals, &qh).context("Failed to bind compositor")?;
    let layer_shell = LayerShell::bind(&globals, &qh).context("Failed to bind layer shell")?;
    let shm = Shm::bind(&globals, &qh).context("Failed to bind shm")?;

    // Get the display pointer for GPU rendering
    let display_ptr = conn.backend().display_ptr() as *mut std::ffi::c_void;

    // Create application state
    let mut app = WaylandApp::new(
        RegistryState::new(&globals),
        SeatState::new(&globals, &qh),
        OutputState::new(&globals, &qh),
        shm,
        layer_shell,
        compositor_state,
        display_ptr,
        image,
        opacity,
        use_gpu,
    );

    // Dispatch once to get output info
    event_queue.roundtrip(&mut app)?;

    // Get display dimensions from outputs
    let (display_width, display_height) = get_display_dimensions(&app.output_state);
    app.display_width = display_width;
    app.display_height = display_height;
    info!("Display dimensions: {}x{}", display_width, display_height);

    // Calculate the target size (limit to 10% of screen area)
    let (target_width, target_height) = calculate_limited_size(
        app.image.width,
        app.image.height,
        display_width,
        display_height,
        0.10,
    );
    info!(
        "Image size: {}x{} -> Display size: {}x{}",
        app.image.width, app.image.height, target_width, target_height
    );

    // Set initial window position (centered)
    app.margin_left = ((display_width - target_width) / 2) as i32;
    app.margin_top = ((display_height - target_height) / 2) as i32;
    app.width = target_width;
    app.height = target_height;

    // Create the layer surface
    let surface = app.compositor_state.create_surface(&qh);
    let layer_surface = app.layer_shell.create_layer_surface(
        &qh,
        surface,
        Layer::Overlay,
        Some("rspin"),
        None,
    );

    // Configure the layer surface with anchoring for positioning
    layer_surface.set_anchor(Anchor::TOP | Anchor::LEFT);
    layer_surface.set_margin(app.margin_top, 0, 0, app.margin_left);
    layer_surface.set_size(target_width, target_height);
    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);

    // Commit the surface to trigger configure
    layer_surface.commit();

    app.layer_surface = Some(layer_surface);

    info!("Starting event loop");
    info!("Controls: Double-click to close, Right-click for menu, Scroll to adjust opacity");
    info!("Drag edges to resize, Drag center to move");

    // Main event loop
    loop {
        event_queue.blocking_dispatch(&mut app)?;

        if app.should_exit {
            info!("Exiting application");
            break;
        }
    }

    Ok(())
}

/// Get display dimensions from the output state
fn get_display_dimensions(output_state: &OutputState) -> (u32, u32) {
    for output in output_state.outputs() {
        if let Some(info) = output_state.info(&output) {
            if let Some(mode) = info.modes.iter().find(|m| m.current) {
                return (mode.dimensions.0 as u32, mode.dimensions.1 as u32);
            }
            if let Some(mode) = info.modes.first() {
                return (mode.dimensions.0 as u32, mode.dimensions.1 as u32);
            }
        }
    }
    (1920, 1080)
}

/// Calculate the display size limited to a percentage of screen area
fn calculate_limited_size(
    img_width: u32,
    img_height: u32,
    screen_width: u32,
    screen_height: u32,
    max_screen_fraction: f32,
) -> (u32, u32) {
    let max_width = (screen_width as f32 * max_screen_fraction.sqrt()) as u32;
    let max_height = (screen_height as f32 * max_screen_fraction.sqrt()) as u32;

    if img_width <= max_width && img_height <= max_height {
        return (img_width, img_height);
    }

    let scale_x = max_width as f32 / img_width as f32;
    let scale_y = max_height as f32 / img_height as f32;
    let scale = scale_x.min(scale_y);

    let new_width = (img_width as f32 * scale) as u32;
    let new_height = (img_height as f32 * scale) as u32;

    (new_width.max(1), new_height.max(1))
}
