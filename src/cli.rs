// Command line interface module
// Handles parsing of command line arguments and stdin input

use anyhow::{bail, Result};
use clap::Parser;
use std::io::{self, Read};
use std::path::PathBuf;

/// rspin - A desktop sticky image viewer for Wayland
#[derive(Parser, Debug)]
#[command(name = "rspin")]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Path to the image file (can also be provided via stdin pipe)
    #[arg(value_name = "IMAGE")]
    pub image_path: Option<PathBuf>,

    /// Opacity of the window (0.0 - 1.0)
    #[arg(short, long, default_value = "1.0", value_parser = parse_opacity)]
    pub opacity: f32,

    /// Initial X position of the window
    #[arg(short = 'x', long)]
    pub pos_x: Option<i32>,

    /// Initial Y position of the window
    #[arg(short = 'y', long)]
    pub pos_y: Option<i32>,

    /// Scale factor for the image (e.g., 0.5 for half size, 2.0 for double)
    #[arg(short, long, default_value = "1.0")]
    pub scale: f32,

    /// Disable GPU rendering and use CPU rendering only
    #[arg(long, default_value = "false")]
    pub cpu: bool,
}

/// Parsed arguments with resolved image source
#[derive(Debug)]
pub struct ParsedArgs {
    pub image_path: Option<PathBuf>,
    pub image_data: Option<Vec<u8>>,
    pub opacity: f32,
    #[allow(dead_code)]
    pub pos_x: Option<i32>,
    #[allow(dead_code)]
    pub pos_y: Option<i32>,
    pub scale: f32,
    /// Use GPU rendering (default true, set to false with --cpu)
    pub use_gpu: bool,
}

/// Parse opacity value and ensure it's within valid range
fn parse_opacity(s: &str) -> Result<f32, String> {
    let opacity: f32 = s.parse().map_err(|_| "Invalid opacity value")?;
    if !(0.0..=1.0).contains(&opacity) {
        return Err("Opacity must be between 0.0 and 1.0".to_string());
    }
    Ok(opacity)
}

/// Check if stdin has data available (is a pipe)
fn stdin_has_data() -> bool {
    !atty::is(atty::Stream::Stdin)
}

/// Read image data from stdin
fn read_stdin() -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    io::stdin().read_to_end(&mut buffer)?;
    Ok(buffer)
}

/// Parse command line arguments and handle stdin input
pub fn parse_args() -> Result<ParsedArgs> {
    let args = Args::parse();

    // Check if we have image data from stdin
    let (image_path, image_data) = if stdin_has_data() {
        // Read from stdin
        let data = read_stdin()?;
        if data.is_empty() {
            bail!("No data received from stdin");
        }
        (args.image_path, Some(data))
    } else if let Some(path) = args.image_path {
        // Use file path
        (Some(path), None)
    } else {
        bail!("No image provided. Please provide an image path or pipe image data to stdin.\n\
               Usage: rspin <IMAGE> [OPTIONS]\n\
               Or:    cat image.png | rspin [OPTIONS]");
    };

    Ok(ParsedArgs {
        image_path,
        image_data,
        opacity: args.opacity,
        pos_x: args.pos_x,
        pos_y: args.pos_y,
        scale: args.scale,
        use_gpu: !args.cpu, // GPU is default, --cpu disables it
    })
}
