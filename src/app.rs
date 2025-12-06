// Application state module
// Manages the overall application state

use crate::image_loader::ImageData;

/// Main application state (reserved for future extensions)
#[allow(dead_code)]
pub struct AppState {
    /// The loaded image data
    pub image: ImageData,
    /// Window opacity (0.0 - 1.0)
    pub opacity: f32,
    /// Whether the application should exit
    pub should_exit: bool,
}

#[allow(dead_code)]
impl AppState {
    /// Create a new application state
    pub fn new(image: ImageData, opacity: f32) -> Self {
        Self {
            image,
            opacity,
            should_exit: false,
        }
    }

    /// Mark the application for exit
    pub fn exit(&mut self) {
        self.should_exit = true;
    }
}
