//! Unified API for Rip receipt markup.
//!
//! Parse `.rip` markup and render to grayscale pixels, 1-bit raster,
//! HTML, plain text, or ESC/POS binary.
//!
//! # Example
//!
//! ```
//! let nodes = rip::parse("## Hello\n---\nItem | $5.00");
//! let resources = rip::RenderResources::default();
//! let output = rip::render_luma8(&nodes, &resources).unwrap();
//! // output.pixels is row-major luma8 (0=black, 255=white)
//! ```

// Re-export core types
pub use rip_parser::{
    parse, collect_resources,
    ImageData, RenderResources, ResourceUrls,
};
pub use rip_parser::ast::Node;
pub use rip_image::RenderError;

/// Pixel output from image rendering.
pub struct PixelOutput {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Pixel data.
    ///
    /// For `render_luma8`: row-major 8-bit grayscale, length = width × height.
    /// For `render_luma1`: 1-bit packed MSB-first, length = ceil(width/8) × height.
    pub pixels: Vec<u8>,
    /// Which rows contain non-background content.
    ///
    /// Length = height. Useful for optimized encoding — the host's PNG
    /// encoder can skip all-white rows.
    pub dirty_rows: Vec<bool>,
}

/// Render to 8-bit grayscale pixels (anti-aliased).
///
/// The host encodes the pixels to PNG, WebP, or whatever format
/// is appropriate for the platform.
pub fn render_luma8(
    nodes: &[Node],
    resources: &RenderResources,
) -> Result<PixelOutput, RenderError> {
    let (width, height, pixels, dirty_rows) =
        rip_image::render_pixels_with_dirty(nodes, resources)?;
    Ok(PixelOutput { width, height, pixels, dirty_rows })
}

/// Render to 1-bit packed pixels (thresholded black/white).
///
/// Output is MSB-first, `ceil(width/8)` bytes per row.
/// Suitable for ESC/POS `GS v 0` raster commands or 1-bit PNG encoding.
pub fn render_luma1(
    nodes: &[Node],
    resources: &RenderResources,
) -> Result<PixelOutput, RenderError> {
    let (width, height, pixels, dirty_rows) =
        rip_image::render_pixels_with_dirty(nodes, resources)?;
    let packed = rip_image::encode_raster(width, &pixels);
    Ok(PixelOutput { width, height, pixels: packed, dirty_rows })
}

/// Render to a standalone HTML document.
///
/// No resources needed — images are referenced by URL in `<img>` tags,
/// QR codes and barcodes are inline SVG.
pub fn render_html(nodes: &[Node]) -> String {
    rip_html::render_html(nodes)
}

/// Render to plain text (monospace ASCII).
///
/// Images, QR codes, and barcodes are rendered as text placeholders.
pub fn render_text(nodes: &[Node]) -> String {
    rip_text::render_text(nodes)
}

/// Render to ESC/POS binary commands for thermal printers.
///
/// Resources are needed for images (pre-decoded grayscale).
/// Fonts are not used — ESC/POS uses the printer's built-in fonts.
pub fn render_escpos(nodes: &[Node], resources: &RenderResources) -> Vec<u8> {
    rip_escpos::render_escpos(nodes, resources)
}

/// Pack 8-bit grayscale pixels into 1-bit raster data.
///
/// Utility for hosts that render with `render_luma8` but need 1-bit
/// output for printer commands. Each byte holds 8 pixels (MSB first),
/// pixels < 128 are set (black).
pub fn pack_luma1(width: u32, pixels: &[u8]) -> Vec<u8> {
    rip_image::encode_raster(width, pixels)
}
