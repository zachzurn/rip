pub mod ast;
pub mod encode;
pub mod inline;
pub mod parser;
pub mod text_util;

use std::collections::HashMap;

pub use ast::*;
pub use parser::parse;

/// Grayscale threshold for black/white rasterization.
///
/// Pixels below this value are considered black (1), at or above are
/// white (0). Shared by all renderers that convert luma8 to 1-bit.
pub const BLACK_THRESHOLD: u8 = 128;

/// Collected resource URLs referenced by a document.
///
/// Returned by [`collect_resources`]. All URLs are deduplicated.
/// The caller fetches these (from disk, network, etc.) and provides
/// the decoded data in a [`RenderResources`] for rendering.
#[derive(Debug, Clone, Default)]
pub struct ResourceUrls {
    /// Font paths from `@style` directives.
    pub fonts: Vec<String>,
    /// Image paths from `@image` directives.
    pub images: Vec<String>,
}

/// Pre-decoded grayscale image provided by the host.
///
/// Pixels are row-major luma8 (0 = black, 255 = white).
/// Length of `pixels` must equal `width * height`.
#[derive(Debug, Clone)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Resolved resources for rendering.
///
/// The host decodes images and loads font files, then passes them here.
/// Renderers fall back to embedded default fonts when a requested font
/// is not present.
#[derive(Debug, Clone, Default)]
pub struct RenderResources {
    /// Pre-decoded grayscale images, keyed by the URL/path from the `.tab` source.
    pub images: HashMap<String, ImageData>,
    /// Raw TTF/OTF font bytes, keyed by the URL/path from `@style` directives.
    pub fonts: HashMap<String, Vec<u8>>,
}

/// Extract all external resource URLs from a parsed document.
///
/// Walks the AST and collects deduplicated font and image URLs so the
/// caller can fetch them (from disk, network, etc.) before rendering.
pub fn collect_resources(nodes: &[Node]) -> ResourceUrls {
    let mut fonts = Vec::new();
    let mut images = Vec::new();

    for node in nodes {
        match node {
            Node::Style { font, .. } => {
                if !fonts.contains(font) {
                    fonts.push(font.clone());
                }
            }
            Node::Image { url, .. } => {
                if !images.contains(url) {
                    images.push(url.clone());
                }
            }
            _ => {}
        }
    }

    ResourceUrls { fonts, images }
}
