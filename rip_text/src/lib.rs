use rip_parser::ast::*;
use rip_parser::text_util::{align_offset, center, divider_char, spans_to_text, word_wrap};
use taffy::prelude::{
    auto, length, AlignItems, Display, FlexDirection, TaffyMaxContent, TaffyTree,
};

/// Compute characters per line from paper width (mm) and font size (pt).
///
/// Uses monospace metrics: char_width = font_height × 0.6,
/// where font_height in mm = points × 25.4 / 72.
///
/// This was tested to get a similar size to what receipt printers
/// would have at our default sizes
fn chars_per_line(paper_width_mm: f64, font_points: f64) -> usize {
    let char_width_mm = font_points * (25.4 / 72.0) * 0.6;
    let chars = (paper_width_mm / char_width_mm).floor() as usize;
    chars.max(10) // safety floor
}

/// Render a parsed Rip AST to plain monospaced text.
pub fn render_text(nodes: &[Node]) -> String {
    let mut paper_width_mm = 80.0;
    let mut font_points = 12.0;

    // Pass 1: collect config
    for node in nodes {
        match node {
            Node::PrinterWidth { mm } => paper_width_mm = *mm,
            Node::Style { level: Size::Text, points, .. } => font_points = *points,
            _ => {}
        }
    }

    let width = chars_per_line(paper_width_mm, font_points);
    let mut out = String::new();

    // Pass 2: render nodes
    for node in nodes {
        render_node(node, width, &mut out);
    }

    out
}

/// Render a single AST node, appending lines to `out`.
fn render_node(node: &Node, width: usize, out: &mut String) {
    match node {
        Node::Text { spans, size } => {
            let text = spans_to_text(spans);
            let is_title = matches!(size, Size::Title | Size::TitleM | Size::TitleL);
            let lines = word_wrap(&text, width);
            for line in &lines {
                if is_title {
                    out.push_str(&center(line, width));
                } else {
                    out.push_str(line);
                }
                out.push('\n');
            }
        }

        Node::Columns { cells, .. } => {
            let cols = column_layout(width, cells.len());
            let mut line = String::new();

            for (i, cell) in cells.iter().enumerate() {
                if i >= cols.len() {
                    break;
                }
                let (col_x, col_w) = cols[i];

                while line.len() < col_x {
                    line.push(' ');
                }

                let cell_text = match &cell.content {
                    CellContent::Spans(spans) => spans_to_text(spans),
                    CellContent::Divider(style) => {
                        let ch = divider_char(*style);
                        std::iter::repeat(ch).take(col_w).collect()
                    }
                };

                let text_len = cell_text.len();
                let offset = align_offset(col_w, text_len, cell.align);

                for _ in 0..offset {
                    line.push(' ');
                }

                if text_len <= col_w.saturating_sub(offset) {
                    line.push_str(&cell_text);
                } else {
                    let max = col_w.saturating_sub(offset);
                    line.push_str(&cell_text[..max]);
                }
            }

            out.push_str(line.trim_end());
            out.push('\n');
        }

        Node::Line { style } => {
            let ch = divider_char(*style);
            let line: String = std::iter::repeat(ch).take(width).collect();
            out.push_str(&line);
            out.push('\n');
        }

        Node::Blank => {
            out.push('\n');
        }

        Node::Image { .. } => {
            out.push_str(&center("[IMAGE]", width));
            out.push('\n');
        }

        Node::Qr { .. } => {
            out.push_str(&center("[QR Code]", width));
            out.push('\n');
        }

        Node::Barcode { .. } => {
            out.push_str(&center("[Barcode]", width));
            out.push('\n');
        }

        Node::Feed { lines } => {
            for _ in 0..*lines {
                out.push('\n');
            }
        }

        // Non-visual nodes
        Node::PrinterWidth { .. }
        | Node::PrinterDpi { .. }
        | Node::PrinterThreshold { .. }
        | Node::Style { .. }
        | Node::Cut { .. }
        | Node::Drawer => {}
    }
}

/// Compute column layout using Taffy flexbox.
///
/// Same flex settings as the image renderer:
///   container: `display: flex; flex-direction: row; align-items: flex-end;`
///   children:  `flex: 1 0 0%;`
fn column_layout(total_width: usize, cell_count: usize) -> Vec<(usize, usize)> {
    if cell_count == 0 {
        return vec![];
    }

    let mut tree: TaffyTree<()> = TaffyTree::new();

    let children: Vec<_> = (0..cell_count)
        .map(|_| {
            tree.new_leaf(taffy::style::Style {
                flex_grow: 1.0,
                flex_shrink: 0.0,
                flex_basis: length(0.0),
                ..Default::default()
            })
            .unwrap()
        })
        .collect();

    let root = tree
        .new_with_children(
            taffy::style::Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                align_items: Some(AlignItems::FlexEnd),
                size: taffy::prelude::Size {
                    width: length(total_width as f32),
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
            (layout.location.x as usize, layout.size.width as usize)
        })
        .collect()
}
