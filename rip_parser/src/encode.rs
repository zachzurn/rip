//! Barcode and QR code encoding.
//!
//! Returns logical grids (bars/modules) — renderers scale these
//! to pixels, SVG, or whatever the output format needs.

use barcoders::sym::code128::Code128;
use barcoders::sym::code39::Code39;
use barcoders::sym::codabar::Codabar;
use barcoders::sym::ean13::EAN13;
use barcoders::sym::ean8::EAN8;
use qrcode::QrCode;

/// A QR code as a 2D grid of modules.
#[derive(Debug, Clone)]
pub struct QrGrid {
    /// Modules per side.
    pub width: u32,
    /// Row-major module data. `true` = dark, `false` = light.
    pub modules: Vec<bool>,
}

/// Encode barcode data using the specified format.
///
/// Returns a 1D bar pattern where each element is `0` (space) or `1` (bar).
pub fn encode_barcode(format: &str, data: &str) -> Option<Vec<u8>> {
    match format.to_uppercase().as_str() {
        "CODE128" => {
            // barcoders requires a character set prefix:
            // À = Set A, Ɓ = Set B (general ASCII), Ć = Set C (numeric pairs)
            // Default to Set B if no prefix is present.
            let prefixed =
                if data.starts_with('À') || data.starts_with('Ɓ') || data.starts_with('Ć') {
                    data.to_string()
                } else {
                    format!("Ɓ{data}")
                };
            Code128::new(&prefixed).ok().map(|b| b.encode())
        }
        "CODE39" => Code39::new(data).ok().map(|b| b.encode()),
        "EAN13" => EAN13::new(data).ok().map(|b| b.encode()),
        "EAN8" => EAN8::new(data).ok().map(|b| b.encode()),
        "CODABAR" => Codabar::new(data).ok().map(|b| b.encode()),
        _ => None,
    }
}

/// Encode data as a QR code grid.
///
/// Returns the module grid. Renderers handle scaling to the desired
/// physical size (pixels, SVG viewBox, etc.).
pub fn encode_qr(data: &str) -> Option<QrGrid> {
    let code = QrCode::new(data.as_bytes()).ok()?;
    let width = code.width() as u32;
    let modules = code
        .to_colors()
        .into_iter()
        .map(|c| c == qrcode::Color::Dark)
        .collect();
    Some(QrGrid { width, modules })
}
