pub mod barcode;
pub mod canvas;
pub mod layout;
pub mod render;
pub mod text;

use rip_parser::ast::Node;
use rip_parser::RenderResources;

/// Errors that can occur during rendering.
#[derive(Debug)]
pub enum RenderError {
    /// No renderable content in the document.
    EmptyDocument,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::EmptyDocument => write!(f, "no renderable content"),
        }
    }
}

/// Render to a raw grayscale pixel buffer.
///
/// Returns `(width, height, pixels)` where pixels is row-major luma8
/// (0 = black, 255 = white).
pub fn render_pixels(
    nodes: &[Node],
    resources: &RenderResources,
) -> Result<(u32, u32, Vec<u8>), RenderError> {
    let ctx = render::RenderContext::new(nodes, resources);
    let (w, h, pixels, _dirty) = ctx.render(nodes)?;
    Ok((w, h, pixels))
}

/// Render to a raw grayscale pixel buffer with dirty row info.
///
/// Returns `(width, height, pixels, dirty_rows)`. Dirty rows indicate
/// which rows contain non-background content — useful for optimized
/// PNG encoding by the host.
pub fn render_pixels_with_dirty(
    nodes: &[Node],
    resources: &RenderResources,
) -> Result<(u32, u32, Vec<u8>, Vec<bool>), RenderError> {
    let ctx = render::RenderContext::new(nodes, resources);
    ctx.render(nodes)
}

/// Render to 1-bit packed raster data for ESC/POS printers.
///
/// Returns `(width, height, data)` where `data` is 1-bit packed rows
/// (MSB first, `ceil(width/8)` bytes per row). Pixels below the
/// threshold (128) are black (1), at or above are white (0).
/// Suitable for `GS v 0` raster print commands.
pub fn render_raster(
    nodes: &[Node],
    resources: &RenderResources,
) -> Result<(u32, u32, Vec<u8>), RenderError> {
    let ctx = render::RenderContext::new(nodes, resources);
    let (width, height, pixels, _dirty) = ctx.render(nodes)?;
    let raster = encode_raster(width, &pixels);
    Ok((width, height, raster))
}

/// Pack a grayscale pixel buffer into 1-bit raster data.
///
/// Each byte holds 8 pixels, MSB first. Pixels < 128 are set (black).
/// Returns `ceil(width/8) * height` bytes.
pub fn encode_raster(width: u32, pixels: &[u8]) -> Vec<u8> {
    let w = width as usize;
    let row_bytes = (w + 7) / 8;
    let height = pixels.len() / w;
    let mut out = vec![0u8; row_bytes * height];

    for y in 0..height {
        let src_row = y * w;
        let dst_row = y * row_bytes;
        for x in 0..w {
            if pixels[src_row + x] < 128 {
                out[dst_row + x / 8] |= 0x80 >> (x % 8);
            }
        }
    }

    out
}
