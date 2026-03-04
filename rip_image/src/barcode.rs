use barcoders::sym::code128::Code128;
use barcoders::sym::code39::Code39;
use barcoders::sym::codabar::Codabar;
use barcoders::sym::ean13::EAN13;
use barcoders::sym::ean8::EAN8;
use qrcode::QrCode;

/// Encode barcode data using the specified format. Returns the encoded bar pattern.
/// Each element is 0 (space) or 1 (bar).
///
/// Duplicated from rip_html — same logic, no HTML dependencies.
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

/// Render a QR code to a grayscale pixel grid.
///
/// Returns `(actual_size, pixels)` where pixels is a row-major grayscale buffer.
/// Dark modules = 0 (black), light modules = 255 (white).
pub fn render_qr(data: &str, target_size_px: u32) -> Option<(u32, Vec<u8>)> {
    let code = QrCode::new(data.as_bytes()).ok()?;
    let modules = code.to_colors();
    let module_count = code.width() as u32;
    let scale = (target_size_px / module_count).max(1);
    let actual_size = module_count * scale;

    let mut pixels = vec![255u8; (actual_size * actual_size) as usize];
    for row in 0..module_count {
        for col in 0..module_count {
            let idx = (row * module_count + col) as usize;
            if modules[idx] == qrcode::Color::Dark {
                for dy in 0..scale {
                    for dx in 0..scale {
                        let px = ((row * scale + dy) * actual_size + col * scale + dx) as usize;
                        pixels[px] = 0; // black
                    }
                }
            }
        }
    }

    Some((actual_size, pixels))
}
