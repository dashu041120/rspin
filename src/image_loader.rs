// Image loading module
// Handles loading and processing of image files

use crate::cli::ParsedArgs;
use anyhow::{Context, Result};
use image::{DynamicImage, ImageFormat};
use std::fs;
use std::io::Cursor;

/// Loaded image data ready for display
#[derive(Debug, Clone)]
pub struct ImageData {
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// Raw RGBA pixel data (4 bytes per pixel)
    pub rgba_data: Vec<u8>,
    /// Applied scale factor
    #[allow(dead_code)]
    pub scale: f32,
    /// Mipmap levels for faster downscaling (progressively half-sized versions)
    pub mipmaps: Vec<MipmapLevel>,
}

/// A single mipmap level
#[derive(Debug, Clone)]
pub struct MipmapLevel {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// Load and process an image from the parsed arguments
pub fn load_image(args: &ParsedArgs) -> Result<ImageData> {
    let img = if let Some(ref data) = args.image_data {
        // Load from raw bytes (stdin)
        load_from_bytes(data)?
    } else if let Some(ref path) = args.image_path {
        // Load from file
        let data = fs::read(path)
            .with_context(|| format!("Failed to read image file: {}", path.display()))?;
        load_from_bytes(&data)?
    } else {
        anyhow::bail!("No image source provided");
    };

    // Apply scaling if needed
    let img = if (args.scale - 1.0).abs() > f32::EPSILON {
        let new_width = (img.width() as f32 * args.scale) as u32;
        let new_height = (img.height() as f32 * args.scale) as u32;
        img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    // Convert to RGBA format
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    // Convert RGBA to BGRA (Wayland expects ARGB/BGRA in little-endian)
    let mut bgra_data = rgba.into_raw();
    for pixel in bgra_data.chunks_exact_mut(4) {
        pixel.swap(0, 2); // Swap R and B
    }

    // Generate mipmaps for faster downscaling
    let mipmaps = generate_mipmaps(width, height, &bgra_data);

    Ok(ImageData {
        width,
        height,
        rgba_data: bgra_data,
        scale: args.scale,
        mipmaps,
    })
}

/// Generate mipmap levels (progressively half-sized versions)
fn generate_mipmaps(width: u32, height: u32, data: &[u8]) -> Vec<MipmapLevel> {
    let mut mipmaps = Vec::new();
    let mut current_width = width;
    let mut current_height = height;
    let mut current_data = data.to_vec();

    // Generate up to 8 levels or until size is too small
    while current_width > 64 && current_height > 64 && mipmaps.len() < 8 {
        let next_width = current_width / 2;
        let next_height = current_height / 2;
        
        if next_width < 32 || next_height < 32 {
            break;
        }

        // Downsample using box filter (2x2 average)
        let mut next_data = vec![0u8; (next_width * next_height * 4) as usize];
        
        for y in 0..next_height {
            for x in 0..next_width {
                let src_x = x * 2;
                let src_y = y * 2;
                
                // Average 2x2 block
                let mut r = 0u32;
                let mut g = 0u32;
                let mut b = 0u32;
                let mut a = 0u32;
                
                for dy in 0..2 {
                    for dx in 0..2 {
                        let sx = (src_x + dx).min(current_width - 1);
                        let sy = (src_y + dy).min(current_height - 1);
                        let idx = ((sy * current_width + sx) * 4) as usize;
                        
                        if idx + 3 < current_data.len() {
                            b += current_data[idx] as u32;
                            g += current_data[idx + 1] as u32;
                            r += current_data[idx + 2] as u32;
                            a += current_data[idx + 3] as u32;
                        }
                    }
                }
                
                let dst_idx = ((y * next_width + x) * 4) as usize;
                if dst_idx + 3 < next_data.len() {
                    next_data[dst_idx] = (b / 4) as u8;
                    next_data[dst_idx + 1] = (g / 4) as u8;
                    next_data[dst_idx + 2] = (r / 4) as u8;
                    next_data[dst_idx + 3] = (a / 4) as u8;
                }
            }
        }
        
        mipmaps.push(MipmapLevel {
            width: next_width,
            height: next_height,
            data: next_data.clone(),
        });
        
        current_width = next_width;
        current_height = next_height;
        current_data = next_data;
    }
    
    mipmaps
}

/// Load an image from raw bytes, auto-detecting the format
fn load_from_bytes(data: &[u8]) -> Result<DynamicImage> {
    // Try to guess the format from the data
    let format = image::guess_format(data).context("Failed to detect image format")?;

    let cursor = Cursor::new(data);
    let img = image::load(cursor, format).context("Failed to decode image")?;

    Ok(img)
}

/// Get the appropriate image format from file extension
#[allow(dead_code)]
pub fn format_from_extension(ext: &str) -> Option<ImageFormat> {
    match ext.to_lowercase().as_str() {
        "png" => Some(ImageFormat::Png),
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "gif" => Some(ImageFormat::Gif),
        "webp" => Some(ImageFormat::WebP),
        "bmp" => Some(ImageFormat::Bmp),
        "ico" => Some(ImageFormat::Ico),
        "tiff" | "tif" => Some(ImageFormat::Tiff),
        _ => None,
    }
}
