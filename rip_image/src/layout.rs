use rip_parser::ast::Align;
use taffy::prelude::*;

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
/// Uses `pt × (dpi/72)^0.65` scaling so text density on the rendered
/// image closely matches real thermal printers. At 203 DPI, 12pt ≈ 23px.
pub fn font_pt_to_px(pt: f64, dpi: f64) -> f32 {
    (pt * (dpi / 72.0_f64).powf(0.65)) as f32
}

/// Calculate the x-offset for content within a cell, given alignment.
pub fn align_offset(cell_width: u32, content_width: u32, align: Align) -> u32 {
    match align {
        Align::Left => 0,
        Align::Right => cell_width.saturating_sub(content_width),
        Align::Center => cell_width.saturating_sub(content_width) / 2,
    }
}

/// Compute column offsets using Taffy flexbox layout.
///
/// Creates a flex-row container of `paper_width` pixels with `cell_count`
/// equal-flex children. Returns `(x_offset, width)` for each column.
pub fn column_layout(paper_width: u32, cell_count: usize) -> Vec<(u32, u32)> {
    if cell_count == 0 {
        return vec![];
    }

    let mut tree: TaffyTree<()> = TaffyTree::new();

    // Mirrors the HTML renderer's CSS:
    //   .cell { flex: 1 0 0%; }
    let children: Vec<_> = (0..cell_count)
        .map(|_| {
            tree.new_leaf(Style {
                flex_grow: 1.0,
                flex_shrink: 0.0,
                flex_basis: length(0.0),
                ..Default::default()
            })
            .unwrap()
        })
        .collect();

    // Mirrors the HTML renderer's CSS:
    //   .row { display: flex; align-items: flex-end; }
    let root = tree
        .new_with_children(
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                align_items: Some(AlignItems::FlexEnd),
                size: taffy::prelude::Size {
                    width: length(paper_width as f32),
                    height: auto(),
                },
                ..Default::default()
            },
            &children,
        )
        .unwrap();

    tree.compute_layout(root, taffy::prelude::Size::MAX_CONTENT)
        .unwrap();

    children
        .iter()
        .map(|&child| {
            let layout = tree.layout(child).unwrap();
            (layout.location.x as u32, layout.size.width as u32)
        })
        .collect()
}
