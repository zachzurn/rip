//! Unified API for Rip receipt markup.
//!
//! Parse `.rip` markup and render to PNG, ESC/POS raster, ESC/POS text,
//! HTML, or plain text. Each render function takes a [`ResourceConfig`]
//! for resource-needing formats (image, raster, ESC/POS) — the library
//! handles all fetching, caching, and processing internally.
//!
//! # Example
//!
//! ```ignore
//! let nodes = rip::parse("## Hello\n---\nItem | $5.00");
//! let config = rip::ResourceConfig {
//!     resource_dir: Some("./assets".into()),
//!     cache_dir: Some("./cache".into()),
//! };
//! let png_bytes = rip::render_image(&nodes, &config).unwrap();
//! ```

// Re-export parser types
pub use rip_parser::{
    parse, collect_resources,
    ResourceUrls, BLACK_THRESHOLD,
};
pub use rip_parser::ast::Node;

// Re-export resource config
pub use rip_resources::ResourceConfig;

pub use rip_image::RenderError;

/// Render to a compressed 1-bit PNG image.
///
/// Fetches fonts and images via the config paths, processes them (decode,
/// scale, dither/threshold), then renders to a 1-bit black/white PNG with
/// maximum compression. Matches thermal printer appearance.
pub fn render_image(
    nodes: &[Node],
    config: &ResourceConfig,
) -> Result<Vec<u8>, RenderError> {
    let resources = rip_resources::prepare_resources(nodes, config);
    let threshold = collect_threshold(nodes);
    let (width, height, pixels, _dirty) =
        rip_image::render_pixels_with_dirty(nodes, &resources)?;
    Ok(rip_image::encode_png(width, height, &pixels, threshold))
}

/// Render to ESC/POS raster print commands.
///
/// Fetches fonts and images via the config paths, processes them, then
/// renders to a complete ESC/POS byte stream: printer initialization
/// (`ESC @`), raster image (`GS v 0`), and a 4-line feed (`ESC d 4`).
/// The output can be sent directly to a thermal printer.
pub fn render_raster(
    nodes: &[Node],
    config: &ResourceConfig,
) -> Result<Vec<u8>, RenderError> {
    let resources = rip_resources::prepare_resources(nodes, config);
    let threshold = collect_threshold(nodes);
    let (width, height, pixels, _dirty) =
        rip_image::render_pixels_with_dirty(nodes, &resources)?;
    let packed = rip_image::encode_raster(width, &pixels, threshold);
    let padded_width = (width + 7) & !7;

    let mut buf = Vec::new();
    // ESC @ — initialize printer
    buf.extend_from_slice(&[0x1B, b'@']);
    // GS v 0 — raster bit image (m=0, normal scale)
    let width_bytes = padded_width / 8;
    let xl = (width_bytes & 0xFF) as u8;
    let xh = ((width_bytes >> 8) & 0xFF) as u8;
    let yl = (height & 0xFF) as u8;
    let yh = ((height >> 8) & 0xFF) as u8;
    buf.extend_from_slice(&[0x1D, b'v', b'0', 0, xl, xh, yl, yh]);
    buf.extend_from_slice(&packed);
    // ESC d 4 — feed 4 lines
    buf.extend_from_slice(&[0x1B, b'd', 4]);

    Ok(buf)
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
/// Fetches fonts and images via the config paths, processes them, then
/// renders using the printer's built-in text engine with ESC/POS formatting.
/// Images are sent inline as raster data via `GS v 0`.
pub fn render_escpos(nodes: &[Node], config: &ResourceConfig) -> Vec<u8> {
    let resources = rip_resources::prepare_resources(nodes, config);
    rip_escpos::render_escpos(nodes, &resources)
}

/// Extract the black/white threshold from nodes, defaulting to `BLACK_THRESHOLD`.
fn collect_threshold(nodes: &[Node]) -> u8 {
    for node in nodes {
        if let Node::PrinterThreshold { threshold } = node {
            return *threshold;
        }
    }
    BLACK_THRESHOLD
}
