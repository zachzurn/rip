//! Floyd-Steinberg error-diffusion dithering for grayscale images.

/// Apply Floyd-Steinberg dithering to a grayscale luma8 pixel buffer in-place.
///
/// After this, every pixel is either 0 (black) or 255 (white).
/// The dithering distributes quantization error to neighboring pixels,
/// producing a halftone-like pattern that preserves tonal detail.
///
/// Pixels are row-major, length must equal `width * height`.
pub fn floyd_steinberg(pixels: &mut [u8], width: u32, height: u32) {
    let w = width as usize;
    let h = height as usize;
    debug_assert_eq!(pixels.len(), w * h);

    // Work buffer with i16 to handle negative error values
    let mut buf: Vec<i16> = pixels.iter().map(|&p| p as i16).collect();

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let old = buf[idx];
            let new_val = if old < 128 { 0i16 } else { 255i16 };
            buf[idx] = new_val;
            let err = old - new_val;

            // Distribute error to neighbors (Floyd-Steinberg coefficients)
            if x + 1 < w {
                buf[idx + 1] += err * 7 / 16;
            }
            if y + 1 < h {
                if x > 0 {
                    buf[(y + 1) * w + x - 1] += err * 3 / 16;
                }
                buf[(y + 1) * w + x] += err * 5 / 16;
                if x + 1 < w {
                    buf[(y + 1) * w + x + 1] += err * 1 / 16;
                }
            }
        }
    }

    // Write back clamped to 0..255
    for (i, p) in pixels.iter_mut().enumerate() {
        *p = buf[i].clamp(0, 255) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_white_stays_white() {
        let mut pixels = vec![255u8; 4 * 4];
        floyd_steinberg(&mut pixels, 4, 4);
        assert!(pixels.iter().all(|&p| p == 255));
    }

    #[test]
    fn all_black_stays_black() {
        let mut pixels = vec![0u8; 4 * 4];
        floyd_steinberg(&mut pixels, 4, 4);
        assert!(pixels.iter().all(|&p| p == 0));
    }

    #[test]
    fn output_is_binary() {
        // 50% gray should produce a mix of 0s and 255s
        let mut pixels = vec![128u8; 8 * 8];
        floyd_steinberg(&mut pixels, 8, 8);
        assert!(pixels.iter().all(|&p| p == 0 || p == 255));
        // Should have roughly half black and half white
        let black_count = pixels.iter().filter(|&&p| p == 0).count();
        assert!(black_count > 16 && black_count < 48, "Expected ~32 black pixels, got {}", black_count);
    }

    #[test]
    fn gradient_preserves_density() {
        // A gradient from black to white should have decreasing black pixel density
        let mut pixels = Vec::with_capacity(256 * 1);
        for x in 0..256 {
            pixels.push(x as u8);
        }
        floyd_steinberg(&mut pixels, 256, 1);
        assert!(pixels.iter().all(|&p| p == 0 || p == 255));

        // Left side (dark) should have more black pixels than right side (bright)
        let left_black = pixels[..128].iter().filter(|&&p| p == 0).count();
        let right_black = pixels[128..].iter().filter(|&&p| p == 0).count();
        assert!(left_black > right_black, "Left ({left_black}) should have more black than right ({right_black})");
    }
}
