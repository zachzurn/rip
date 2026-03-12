use fontdue::Font;
use rip_parser::ast::{Cell, CellContent};

use crate::text;

/// Convert millimeters to pixels at the given DPI.
pub fn mm_to_px(mm: f64, dpi: f64) -> u32 {
    (mm * dpi / 25.4).round() as u32
}

/// Compute the printable width in pixels, accounting for non-printable margins.
///
/// Standard thermal printers have ~4mm non-printable margin on each side.
/// 80mm paper → 576px, 58mm paper → 400px at 203 DPI.
pub fn printable_px(paper_width_mm: f64, dpi: f64) -> u32 {
    let margin_mm = 4.0;
    let printable_mm = (paper_width_mm - margin_mm * 2.0).max(paper_width_mm * 0.5);
    (printable_mm * dpi / 25.4).round().max(8.0) as u32
}

/// Convert typographic points to pixels at the given DPI.
///
/// Used for physical dimensions (image widths, QR sizes, etc.) where
/// points map directly to inches (1pt = 1/72 in).
pub fn pt_to_px(pt: f64, dpi: f64) -> f32 {
    (pt * dpi / 72.0) as f32
}

/// Convert font point size to rasterization pixels at the given DPI.
///
/// Uses `pt × (dpi/72)^0.75` scaling so text density on the rendered
/// image closely matches real thermal printers. At 203 DPI, 12pt ≈ 27px.
pub fn font_pt_to_px(pt: f64, dpi: f64) -> f32 {
    (pt * (dpi / 72.0_f64).powf(0.75)) as f32
}

/// Calculate the x-offset for content within a cell, given alignment.
pub fn align_offset(cell_width: u32, content_width: u32, align: rip_parser::ast::Align) -> u32 {
    match align {
        rip_parser::ast::Align::Left => 0,
        rip_parser::ast::Align::Right => cell_width.saturating_sub(content_width),
        rip_parser::ast::Align::Center => cell_width.saturating_sub(content_width) / 2,
    }
}

/// Computed layout for a single column cell.
pub struct CellLayout {
    pub x: u32,
    pub width: u32,
}

/// Result of a column layout computation.
pub struct ColumnLayout {
    pub cells: Vec<CellLayout>,
    pub row_height: u32,
}

/// Compute column layout with pre-computed widths and content-aware wrapped heights.
///
/// Uses the provided `(x, width)` pairs for column positions, then measures
/// text wrapping to determine the row height.
pub fn column_layout_wrapped(
    cols: &[(f64, f64)],
    cells: &[Cell],
    font: &Font,
    size_px: f32,
    bold_font: Option<(&Font, f32)>,
) -> ColumnLayout {
    if cells.is_empty() || cols.is_empty() {
        return ColumnLayout { cells: vec![], row_height: 0 };
    }

    let lh = text::line_height(font, size_px);

    // Measure wrapped height for each cell
    let mut max_lines = 1u32;
    for (i, cell) in cells.iter().enumerate() {
        if i >= cols.len() {
            break;
        }
        let col_w = cols[i].1 as f32;
        if let CellContent::Spans(spans) = &cell.content {
            let num_lines = text::count_lines_wrapped(spans, font, size_px, col_w, bold_font);
            max_lines = max_lines.max(num_lines);
        }
    }

    let row_height = (lh * max_lines as f32).ceil() as u32;

    let cell_layouts = cols
        .iter()
        .map(|&(x, w)| CellLayout {
            x: x.round() as u32,
            width: w.ceil() as u32,
        })
        .collect();

    ColumnLayout {
        cells: cell_layouts,
        row_height,
    }
}
