// rspin - A desktop sticky image viewer for Wayland
// Displays an image in a floating, always-on-top window with customizable opacity

mod app;
mod cli;
mod image_loader;
mod wayland;
mod wgpu_renderer;

use anyhow::Result;
use log::info;

fn main() -> Result<()> {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Parse command line arguments
    let args = cli::parse_args()?;

    info!(
        "Starting rspin with image: {:?}, opacity: {}",
        args.image_path, args.opacity
    );

    // Load the image
    let image_data = image_loader::load_image(&args)?;

    info!(
        "Image loaded: {}x{} pixels",
        image_data.width, image_data.height
    );

    // Run with layer-shell (GPU rendering by default, CPU as fallback)
    info!("Using layer-shell overlay mode (GPU: {})", args.use_gpu);
    wayland::run(image_data, args.opacity, args.use_gpu)
}
