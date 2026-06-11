//! QR-code rendering for the Web Drop screen.
//!
//! Encodes a short local URL into a PNG (white background, black modules,
//! generous quiet zone) so an iPhone camera can scan it instantly. Everything
//! is generated in memory — no network, no external services.

use anyhow::{Context, Result};
use image::{ImageBuffer, Luma};
use qrcode::{Color, QrCode};

/// Render `data` (typically `http://<lan-ip>:<port>/`) into PNG bytes.
///
/// `module_px` is the pixel size of one QR module; `quiet` is the white
/// border in modules (the spec recommends at least 4).
pub fn png_bytes(data: &str, module_px: u32, quiet: u32) -> Result<Vec<u8>> {
    let code = QrCode::new(data.as_bytes()).context("encode QR code")?;
    let width = code.width() as u32;
    let colors = code.to_colors();

    let img_dim = (width + quiet * 2) * module_px;
    let mut img = ImageBuffer::from_pixel(img_dim, img_dim, Luma([255u8]));

    for y in 0..width {
        for x in 0..width {
            if colors[(y * width + x) as usize] == Color::Dark {
                let px = (x + quiet) * module_px;
                let py = (y + quiet) * module_px;
                for dy in 0..module_px {
                    for dx in 0..module_px {
                        img.put_pixel(px + dx, py + dy, Luma([0u8]));
                    }
                }
            }
        }
    }

    let mut out = Vec::new();
    image::DynamicImage::ImageLuma8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageOutputFormat::Png)
        .context("encode PNG")?;
    Ok(out)
}
