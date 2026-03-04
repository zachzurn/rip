use rip_parser::encode;

/// Render a QR code to a grayscale pixel grid.
///
/// Uses `rip_parser::encode::encode_qr` for the logical module grid,
/// then scales modules to pixels for the image renderer.
/// Returns `(actual_size, pixels)` where pixels is a row-major grayscale buffer.
/// Dark modules = 0 (black), light modules = 255 (white).
pub fn render_qr(data: &str, target_size_px: u32) -> Option<(u32, Vec<u8>)> {
    let grid = encode::encode_qr(data)?;
    let scale = (target_size_px / grid.width).max(1);
    let actual_size = grid.width * scale;

    let mut pixels = vec![255u8; (actual_size * actual_size) as usize];
    for row in 0..grid.width {
        for col in 0..grid.width {
            if grid.modules[(row * grid.width + col) as usize] {
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
