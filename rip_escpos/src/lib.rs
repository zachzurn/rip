//! ESC/POS binary renderer for Rip markup.
//!
//! Produces a `Vec<u8>` of raw ESC/POS commands that can be sent
//! directly to a thermal printer. Uses `GS !` width/height multipliers
//! to render the six Rip size levels as physically larger text.
//! Images are sent as raster data via `GS v 0`.

mod cmd;

use rip_parser::ast::*;
use rip_parser::text_util::{align_offset, column_widths, divider_char, spans_to_text, word_wrap, precompute_columns_chars};
use rip_parser::BLACK_THRESHOLD;
use rip_resources::RenderResources;

/// Map a Rip Size to ESC/POS width and height multipliers (1–8).
fn size_multiplier(size: Size) -> (u8, u8) {
    match size {
        Size::Text => (1, 1),
        Size::TextM => (2, 1),
        Size::TextL => (2, 2),
        Size::Title => (2, 2),
        Size::TitleM => (3, 3),
        Size::TitleL => (4, 4),
    }
}

/// Whether a size level is a title (centered).
fn is_title(size: Size) -> bool {
    matches!(size, Size::Title | Size::TitleM | Size::TitleL)
}

// ─── Layout helpers ──────────────────────────────────────────────────

/// Compute characters per line for ESC/POS output.
///
/// Uses standard thermal printer Font A metrics: 12 dots wide at 203 DPI.
/// Assumes ~4mm non-printable margin on each side (standard for Epson,
/// Star, etc.), giving 48 chars on 80mm paper and 32 on 58mm paper.
fn chars_per_line(paper_width_mm: f64, dpi: f64) -> usize {
    let dots = printable_dots(paper_width_mm, dpi);
    let font_a_width = 12u32;
    (dots / font_a_width).max(10) as usize
}

/// Compute the printable width in dots.
///
/// Standard thermal printers have ~4mm non-printable margin on each side.
/// 80mm paper → 576 dots, 58mm paper → 400 dots at 203 DPI.
fn printable_dots(paper_width_mm: f64, dpi: f64) -> u32 {
    let margin_mm = 4.0;
    let printable_mm = (paper_width_mm - margin_mm * 2.0).max(paper_width_mm * 0.5);
    (printable_mm * dpi / 25.4).round().max(8.0) as u32
}


// ─── Image helpers ───────────────────────────────────────────────────

/// Convert points to dots at the given DPI.
fn pt_to_dots(pt: f64, dpi: f64) -> u32 {
    (pt * dpi / 72.0).round() as u32
}

/// Compute scaled dimensions to fit within max bounds, preserving aspect ratio.
///
/// Will not upscale beyond natural size when only one bound is specified.
fn scale_dims(nat_w: u32, nat_h: u32, max_w: u32, max_h: Option<u32>) -> (u32, u32) {
    let (w, h) = match max_h {
        Some(mh) => {
            let scale = (max_w as f32 / nat_w as f32).min(mh as f32 / nat_h as f32);
            ((nat_w as f32 * scale) as u32, (nat_h as f32 * scale) as u32)
        }
        None => {
            let scale = (max_w as f32 / nat_w as f32).min(1.0);
            ((nat_w as f32 * scale) as u32, (nat_h as f32 * scale) as u32)
        }
    };
    (w.max(1), h.max(1))
}

/// Nearest-neighbor scale a grayscale pixel buffer.
fn scale_nn(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let mut out = vec![255u8; (dst_w * dst_h) as usize];
    for dy in 0..dst_h {
        let sy = (dy as u64 * src_h as u64 / dst_h as u64) as usize;
        for dx in 0..dst_w {
            let sx = (dx as u64 * src_w as u64 / dst_w as u64) as usize;
            out[(dy * dst_w + dx) as usize] = src[sy * src_w as usize + sx];
        }
    }
    out
}

/// Threshold a grayscale pixel buffer to 1-bit and pack into raster bytes.
///
/// Returns `(padded_width, height, packed_data)` where `padded_width`
/// is a multiple of 8. Each byte is MSB-first: bit 7 = leftmost pixel.
/// A set bit (1) = black, cleared bit (0) = white.
fn to_raster(pixels: &[u8], width: u32, height: u32, threshold: u8) -> (u32, u32, Vec<u8>) {
    let padded_width = (width + 7) & !7;
    let bytes_per_row = (padded_width / 8) as usize;
    let w = width as usize;

    let mut data = vec![0u8; bytes_per_row * height as usize];

    for y in 0..height as usize {
        for x in 0..w {
            let pixel = pixels[y * w + x];
            if pixel < threshold {
                let byte_idx = y * bytes_per_row + x / 8;
                let bit_idx = 7 - (x % 8);
                data[byte_idx] |= 1 << bit_idx;
            }
        }
    }

    (padded_width, height, data)
}

// ─── ESC/POS rendering ─────────────────────────────────────────────

/// Render a parsed Rip AST to an ESC/POS byte stream.
///
/// The `resources` provide pre-decoded images keyed by their URL/path
/// (matching the values from [`rip_parser::collect_resources`]).
pub fn render_escpos(nodes: &[Node], resources: &RenderResources) -> Vec<u8> {
    let mut paper_width_mm = 80.0;
    let mut dpi = 203.0;
    let mut threshold = BLACK_THRESHOLD;

    // Pass 1: collect config
    for node in nodes {
        match node {
            Node::PrinterWidth { mm } => paper_width_mm = *mm,
            Node::PrinterDpi { dpi: d } => dpi = *d as f64,
            Node::PrinterThreshold { threshold: t } => threshold = *t,
            _ => {}
        }
    }

    let base_width = chars_per_line(paper_width_mm, dpi);
    let max_dots = printable_dots(paper_width_mm, dpi);
    let mut buf = Vec::new();

    // Pre-compute column widths for all groups
    let col_cache = precompute_columns_chars(nodes, base_width, 2.0, 1.0);

    // Initialize printer
    cmd::init(&mut buf);

    // Pass 2: render nodes
    for (i, node) in nodes.iter().enumerate() {
        render_node(node, base_width, max_dots, dpi, threshold, resources, col_cache.get(&i), &mut buf);
    }

    buf
}

/// Render a single AST node, appending ESC/POS bytes to `buf`.
fn render_node(
    node: &Node,
    base_width: usize,
    max_dots: u32,
    dpi: f64,
    threshold: u8,
    resources: &RenderResources,
    precomputed_cols: Option<&Vec<(f64, f64)>>,
    buf: &mut Vec<u8>,
) {
    match node {
        Node::Text { spans, size } => {
            let (w_mult, h_mult) = size_multiplier(*size);
            let effective_width = base_width / w_mult as usize;
            let title = is_title(*size);

            // Set character size
            if w_mult > 1 || h_mult > 1 {
                cmd::char_size(buf, w_mult, h_mult);
            }
            if title {
                cmd::justify(buf, 1); // center
            }

            let text = spans_to_text(spans);
            let lines = word_wrap(&text, effective_width);

            for line in &lines {
                if title {
                    // Justification handles centering, just emit styled spans
                    emit_styled_line(spans, &text, line, buf);
                } else {
                    emit_styled_line(spans, &text, line, buf);
                }
                cmd::linefeed(buf);
            }

            // Reset
            if title {
                cmd::justify(buf, 0); // left
            }
            if w_mult > 1 || h_mult > 1 {
                cmd::char_size(buf, 1, 1);
            }
        }

        Node::Columns { cells, size } => {
            let (w_mult, h_mult) = size_multiplier(*size);
            let effective_width = base_width / w_mult as usize;

            if w_mult > 1 || h_mult > 1 {
                cmd::char_size(buf, w_mult, h_mult);
            }

            let cols: Vec<(usize, usize)> = if w_mult > 1 {
                // For size-multiplied text, recompute at effective width so
                // natural content widths (in chars) are preserved correctly.
                // Scaling pre-computed base-width values would shrink columns
                // below their content width (e.g. "TOTAL" at 2x → 5/2=3 chars).
                let pcts: Vec<Option<u32>> = cells.iter().map(|c| c.width_pct).collect();
                let natural: Vec<f64> = cells.iter().map(|c| {
                    match &c.content {
                        CellContent::Spans(spans) => spans_to_text(spans).len() as f64,
                        CellContent::Divider(_) => 0.0,
                    }
                }).collect();
                column_widths(&pcts, &natural, effective_width as f64, 2.0, 1.0)
                    .into_iter()
                    .map(|(x, w)| (x.round() as usize, w.round() as usize))
                    .collect()
            } else if let Some(pre) = precomputed_cols {
                pre.iter().map(|&(x, w)| {
                    (x.round() as usize, w.round() as usize)
                }).collect()
            } else {
                let pcts: Vec<Option<u32>> = cells.iter().map(|c| c.width_pct).collect();
                let natural = vec![0.0; cells.len()];
                column_widths(&pcts, &natural, effective_width as f64, 2.0, 1.0)
                    .into_iter()
                    .map(|(x, w)| (x.round() as usize, w.round() as usize))
                    .collect()
            };
            emit_wrapped_columns(cells, &cols, effective_width, buf);

            if w_mult > 1 || h_mult > 1 {
                cmd::char_size(buf, 1, 1);
            }
        }

        Node::Line { style } => {
            let ch = divider_char(*style);
            let line: String = std::iter::repeat(ch).take(base_width).collect();
            buf.extend_from_slice(line.as_bytes());
            cmd::linefeed(buf);
        }

        Node::Blank => {
            cmd::linefeed(buf);
        }

        Node::Image {
            url,
            width,
            height,
            align,
            ..
        } => {
            if let Some(img) = resources.images.get(url.as_str()) {
                // Compute max bounds in dots
                let img_max_w = width
                    .map(|w| pt_to_dots(w, dpi).min(max_dots))
                    .unwrap_or(max_dots);
                let img_max_h = height.map(|h| pt_to_dots(h, dpi));

                let (scaled_w, scaled_h) = scale_dims(img.width, img.height, img_max_w, img_max_h);
                let scaled = scale_nn(&img.pixels, img.width, img.height, scaled_w, scaled_h);
                let (raster_w, raster_h, raster_data) = to_raster(&scaled, scaled_w, scaled_h, threshold);

                // Set alignment
                let needs_justify =
                    matches!(align, Some(Align::Center) | Some(Align::Right));
                if let Some(a) = align {
                    let j = match a {
                        Align::Left => 0,
                        Align::Center => 1,
                        Align::Right => 2,
                    };
                    cmd::justify(buf, j);
                }

                cmd::raster_image(buf, raster_w, raster_h, &raster_data);

                if needs_justify {
                    cmd::justify(buf, 0);
                }
            }
        }

        Node::Qr { data, size, align } => {
            // Set alignment if specified
            let needs_justify = matches!(align, Some(Align::Center) | Some(Align::Right));
            if let Some(a) = align {
                let j = match a {
                    Align::Left => 0,
                    Align::Center => 1,
                    Align::Right => 2,
                };
                cmd::justify(buf, j);
            }

            // Map the optional size (in points) to a module size (1–8).
            // Default to 6 which is a good size for most receipts.
            let module_size = match size {
                Some(s) => ((*s / 30.0).round() as u8).max(1).min(8),
                None => 6,
            };

            cmd::qr(buf, data, module_size);

            if needs_justify {
                cmd::justify(buf, 0);
            }
        }

        Node::Barcode { format, data } => {
            cmd::justify(buf, 1); // center barcodes
            cmd::barcode(buf, format, data);
            cmd::justify(buf, 0);
        }

        Node::Feed { amount, unit } => {
            match unit {
                FeedUnit::Lines => {
                    // Integer lines → ESC d (line feed), fractional → ESC J (dot feed)
                    if *amount == amount.round() && *amount <= 255.0 {
                        cmd::feed(buf, *amount as u8);
                    } else {
                        // Convert fractional lines to dots: line height ≈ 30 dots at 203 DPI
                        let line_dots = (dpi / 203.0 * 30.0).round();
                        let dots = (amount * line_dots).round().min(255.0).max(0.0) as u8;
                        cmd::feed_dots(buf, dots);
                    }
                }
                FeedUnit::Mm => {
                    let dots = (amount * dpi / 25.4).round().min(255.0).max(0.0) as u8;
                    cmd::feed_dots(buf, dots);
                }
            }
        }

        Node::Cut { partial } => {
            cmd::cut(buf, *partial);
        }

        Node::Drawer => {
            cmd::drawer(buf);
        }

        // Config nodes — consumed in pass 1
        Node::PrinterWidth { .. }
        | Node::PrinterDpi { .. }
        | Node::PrinterThreshold { .. }
        | Node::Style { .. } => {}
    }
}

/// Emit styled text spans for a single wrapped line of a Text node.
///
/// For simple single-line text, this emits style commands around each span.
/// For wrapped text, we emit the line content with styles applied to the
/// portions that fall within the current line.
fn emit_styled_line(spans: &[Span], full_text: &str, line: &str, buf: &mut Vec<u8>) {
    // If the full text equals the line (no wrapping), emit spans directly
    if full_text == line {
        for span in spans {
            set_span_style(buf, &span.style, true);
            buf.extend_from_slice(span.text.as_bytes());
            set_span_style(buf, &span.style, false);
        }
        return;
    }

    // For wrapped lines, find where this line starts in the full text
    // and emit the appropriate style commands.
    // Simplified: emit as plain text (styles are less meaningful across word-wrap breaks)
    buf.extend_from_slice(line.as_bytes());
}

/// Emit columns with word-wrapping support.
///
/// Each text cell is word-wrapped to its column width. The row spans
/// as many output lines as the tallest cell requires.
fn emit_wrapped_columns(
    cells: &[Cell],
    cols: &[(usize, usize)],
    _effective_width: usize,
    buf: &mut Vec<u8>,
) {
    // Wrap each cell's text and collect lines per cell.
    let mut cell_lines: Vec<Vec<String>> = Vec::with_capacity(cells.len());
    let mut max_lines = 1usize;

    for (i, cell) in cells.iter().enumerate() {
        if i >= cols.len() {
            cell_lines.push(vec![]);
            continue;
        }
        let (_col_x, col_w) = cols[i];

        match &cell.content {
            CellContent::Spans(spans) => {
                let text = spans_to_text(spans);
                let lines = word_wrap(&text, col_w);
                max_lines = max_lines.max(lines.len());
                cell_lines.push(lines);
            }
            CellContent::Divider(_) => {
                // Dividers are single-line
                cell_lines.push(vec![]);
            }
        }
    }

    // Emit one output line per wrapped row.
    for row in 0..max_lines {
        let mut cursor = 0usize;

        for (i, cell) in cells.iter().enumerate() {
            if i >= cols.len() {
                break;
            }
            let (col_x, col_w) = cols[i];

            // Pad to column start
            while cursor < col_x {
                buf.push(b' ');
                cursor += 1;
            }

            match &cell.content {
                CellContent::Spans(spans) => {
                    let line_text = cell_lines[i]
                        .get(row)
                        .map(|s| s.as_str())
                        .unwrap_or("");
                    let text_len = line_text.len();
                    let offset = align_offset(col_w, text_len, cell.align);

                    // Pad for alignment
                    for _ in 0..offset {
                        buf.push(b' ');
                        cursor += 1;
                    }

                    // For the first line, emit styled spans; for wrapped lines, plain text
                    if row == 0 {
                        let available = col_w.saturating_sub(offset);
                        let mut chars_remaining = available.min(text_len);
                        for span in spans {
                            if chars_remaining == 0 {
                                break;
                            }
                            let span_len = span.text.len().min(chars_remaining);
                            set_span_style(buf, &span.style, true);
                            buf.extend_from_slice(&span.text.as_bytes()[..span_len]);
                            set_span_style(buf, &span.style, false);
                            cursor += span_len;
                            chars_remaining -= span_len;
                        }
                    } else {
                        buf.extend_from_slice(line_text.as_bytes());
                        cursor += text_len;
                    }
                }
                CellContent::Divider(style) => {
                    // Only draw divider on the last row (baseline-aligned)
                    if row == max_lines - 1 {
                        let ch = divider_char(*style);
                        for _ in 0..col_w {
                            buf.push(ch as u8);
                            cursor += 1;
                        }
                    }
                }
            }
        }

        cmd::linefeed(buf);
    }
}

/// Set or reset a span style via ESC/POS commands.
fn set_span_style(buf: &mut Vec<u8>, style: &SpanStyle, on: bool) {
    match style {
        SpanStyle::Bold => cmd::bold(buf, on),
        SpanStyle::Underline => cmd::underline(buf, on),
        SpanStyle::Italic => cmd::italic(buf, on),
        SpanStyle::Strikethrough => {} // No ESC/POS equivalent
        SpanStyle::Normal => {}
    }
}
