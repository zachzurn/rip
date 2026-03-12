pub mod ast;
pub mod encode;
pub mod inline;
pub mod parser;
pub mod text_util;

pub use ast::*;
pub use parser::parse;

/// Default grayscale threshold for black/white rasterization.
///
/// Pixels below this value are considered black (1), at or above are
/// white (0). Can be overridden per-document via `@printer-threshold()`.
pub const BLACK_THRESHOLD: u8 = 128;

/// Collected resource URLs referenced by a document.
///
/// Returned by [`collect_resources`]. All URLs are deduplicated.
/// The caller fetches these (from disk, network, etc.) and provides
/// the decoded data for rendering.
#[derive(Debug, Clone, Default)]
pub struct ResourceUrls {
    /// Font paths from `@style` directives.
    pub fonts: Vec<String>,
    /// Image paths from `@image` directives.
    pub images: Vec<String>,
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
