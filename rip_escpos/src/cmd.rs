//! ESC/POS byte-level command helpers.
//!
//! Each function appends the raw command bytes to a `Vec<u8>` buffer.
//! Command references from the user's `thermal` project at
//! `thermal_parser/src/commands/`.

const ESC: u8 = 0x1B;
const GS: u8 = 0x1D;
const LF: u8 = 0x0A;

/// ESC @ — Initialize printer (reset to defaults).
pub fn init(buf: &mut Vec<u8>) {
    buf.extend_from_slice(&[ESC, b'@']);
}

/// 0x0A — Line feed.
pub fn linefeed(buf: &mut Vec<u8>) {
    buf.push(LF);
}

/// ESC a n — Set justification.
///   0 = left, 1 = center, 2 = right.
pub fn justify(buf: &mut Vec<u8>, align: u8) {
    buf.extend_from_slice(&[ESC, b'a', align]);
}

/// ESC E n — Set emphasis (bold).
///   Bit 0: 1 = on, 0 = off.
pub fn bold(buf: &mut Vec<u8>, on: bool) {
    buf.extend_from_slice(&[ESC, b'E', on as u8]);
}

/// ESC - n — Set underline.
///   0 = off, 1 = single, 2 = double.
pub fn underline(buf: &mut Vec<u8>, on: bool) {
    buf.extend_from_slice(&[ESC, b'-', on as u8]);
}

/// ESC 4 n — Set italic.
///   Bit 0: 1 = on, 0 = off.
pub fn italic(buf: &mut Vec<u8>, on: bool) {
    buf.extend_from_slice(&[ESC, 0x34, on as u8]);
}

/// GS ! n — Set character size.
///   Bits 4-6: width multiplier − 1 (0–7).
///   Bits 0-2: height multiplier − 1 (0–7).
///   e.g. w=2, h=2 → ((2-1) << 4) | (2-1) = 0x11
pub fn char_size(buf: &mut Vec<u8>, width: u8, height: u8) {
    let w = (width.saturating_sub(1)).min(7);
    let h = (height.saturating_sub(1)).min(7);
    buf.extend_from_slice(&[GS, b'!', (w << 4) | h]);
}

/// ESC d n — Print and feed n lines.
pub fn feed(buf: &mut Vec<u8>, lines: u8) {
    buf.extend_from_slice(&[ESC, b'd', lines]);
}

/// ESC J n — Print and feed paper n dots (0–255).
///
/// Used for sub-line-height vertical spacing.
pub fn feed_dots(buf: &mut Vec<u8>, dots: u8) {
    buf.extend_from_slice(&[ESC, b'J', dots]);
}

/// GS V m n — Paper cut (Function B).
///   m = 65 (full cut) or 66 (partial cut), n = 0.
///   Function B auto-feeds to the cutter position before cutting,
///   which is required on printers where the cutter is below the print head.
pub fn cut(buf: &mut Vec<u8>, partial: bool) {
    let m = if partial { 66 } else { 65 };
    buf.extend_from_slice(&[GS, b'V', m, 0]);
}

/// ESC p m t1 t2 — Generate pulse (open cash drawer).
///   Pin 0, on-time 25×2ms = 50ms, off-time 255×2ms = 510ms.
pub fn drawer(buf: &mut Vec<u8>) {
    buf.extend_from_slice(&[ESC, b'p', 0x00, 0x19, 0xFF]);
}

/// GS v 0 — Print raster bit image.
///
/// Format: `1D 76 30 m xL xH yL yH d[0]..d[k-1]`
///   - `m` = 0 (normal 1:1 scale)
///   - `xL/xH` = width in **bytes** (width_px / 8), 16-bit little-endian
///   - `yL/yH` = height in dots, 16-bit little-endian
///   - `data` = row-major raster, MSB = leftmost pixel, 1 = black
///
/// The caller must ensure `width_px` is a multiple of 8 and
/// `data.len() == (width_px / 8) * height`.
pub fn raster_image(buf: &mut Vec<u8>, width_px: u32, height: u32, data: &[u8]) {
    let width_bytes = width_px / 8;
    let xl = (width_bytes & 0xFF) as u8;
    let xh = ((width_bytes >> 8) & 0xFF) as u8;
    let yl = (height & 0xFF) as u8;
    let yh = ((height >> 8) & 0xFF) as u8;

    buf.extend_from_slice(&[GS, b'v', b'0', 0, xl, xh, yl, yh]);
    buf.extend_from_slice(data);
}

/// Emit a native QR code using the GS ( k subcommand sequence.
///
/// Sequence:
///   1. Set model (model 2)
///   2. Set module size
///   3. Set error correction (level L)
///   4. Store data
///   5. Print stored symbol
pub fn qr(buf: &mut Vec<u8>, data: &str, module_size: u8) {
    let size = module_size.max(1).min(8);

    // Function 165: Select model — GS ( k 4 0 49 65 50 0
    //   pL=4 pH=0, cn=49, fn=65, model=50 (model 2), nul=0
    buf.extend_from_slice(&[GS, b'(', b'k', 4, 0, 49, 65, 50, 0]);

    // Function 167: Set module size — GS ( k 3 0 49 67 <size>
    buf.extend_from_slice(&[GS, b'(', b'k', 3, 0, 49, 67, size]);

    // Function 169: Set error correction — GS ( k 3 0 49 69 48
    //   48 = error correction level L
    buf.extend_from_slice(&[GS, b'(', b'k', 3, 0, 49, 69, 48]);

    // Function 180: Store data — GS ( k pL pH 49 80 48 <data>
    //   pL/pH = (data.len() + 3) as u16 little-endian
    //   48 = QR code symbol type identifier
    let store_len = (data.len() + 3) as u16;
    let pl = (store_len & 0xFF) as u8;
    let ph = (store_len >> 8) as u8;
    buf.extend_from_slice(&[GS, b'(', b'k', pl, ph, 49, 80, 48]);
    buf.extend_from_slice(data.as_bytes());

    // Function 181: Print symbol — GS ( k 3 0 49 81 48
    //   m=48 (0x30) identifies the QR code symbology to print
    buf.extend_from_slice(&[GS, b'(', b'k', 3, 0, 49, 81, 48]);
}

/// Emit a native barcode using GS k (format B: explicit length).
///
/// Format string is mapped to the ESC/POS type byte (65–73).
/// Returns false if the format is not recognized.
pub fn barcode(buf: &mut Vec<u8>, format: &str, data: &str) -> bool {
    let type_byte = match format.to_uppercase().as_str() {
        "UPC-A" | "UPCA" => 65,
        "UPC-E" | "UPCE" => 66,
        "EAN13" => 67,
        "EAN8" => 68,
        "CODE39" => 69,
        "ITF" => 70,
        "CODABAR" => 71,
        "CODE93" => 72,
        "CODE128" => 73,
        _ => return false,
    };

    // CODE128 requires a character set prefix: {B for Set B (general ASCII)
    let prefixed;
    let data_bytes = if type_byte == 73 && !data.starts_with('{') {
        prefixed = format!("{{B{data}");
        prefixed.as_bytes()
    } else {
        data.as_bytes()
    };

    let len = data_bytes.len().min(255) as u8;

    // GS k m n d1...dn (format B: type 65+, explicit length)
    buf.extend_from_slice(&[GS, b'k', type_byte, len]);
    buf.extend_from_slice(&data_bytes[..len as usize]);

    true
}
