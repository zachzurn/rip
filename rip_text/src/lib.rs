use rip_parser::ast::*;
use rip_parser::text_util::{align_offset, center, column_widths, divider_char, spans_to_text, word_wrap, precompute_columns_chars};

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

    // Pre-compute column widths for all groups
    let col_cache = precompute_columns_chars(nodes, width, 2.0, 1.0);

    let mut out = String::new();

    // Pass 2: render nodes
    for (i, node) in nodes.iter().enumerate() {
        render_node(node, width, col_cache.get(&i), &mut out);
    }

    out
}

/// Render a single AST node, appending lines to `out`.
fn render_node(node: &Node, width: usize, precomputed_cols: Option<&Vec<(f64, f64)>>, out: &mut String) {
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
            let cols: Vec<(usize, usize)> = if let Some(pre) = precomputed_cols {
                pre.iter().map(|&(x, w)| (x.round() as usize, w.round() as usize)).collect()
            } else {
                // Fallback: equal widths, no gap
                let pcts: Vec<Option<u32>> = cells.iter().map(|c| c.width_pct).collect();
                let natural = vec![0.0; cells.len()];
                column_widths(&pcts, &natural, width as f64, 2.0, 1.0)
                    .into_iter()
                    .map(|(x, w)| (x.round() as usize, w.round() as usize))
                    .collect()
            };

            // Word-wrap each cell and determine max lines needed.
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
                        cell_lines.push(vec![]);
                    }
                }
            }

            // Emit one output line per wrapped row.
            for row in 0..max_lines {
                let mut line = String::new();

                for (i, cell) in cells.iter().enumerate() {
                    if i >= cols.len() {
                        break;
                    }
                    let (col_x, col_w) = cols[i];

                    while line.len() < col_x {
                        line.push(' ');
                    }

                    match &cell.content {
                        CellContent::Spans(_) => {
                            let cell_text = cell_lines[i]
                                .get(row)
                                .map(|s| s.as_str())
                                .unwrap_or("");
                            let text_len = cell_text.len();
                            let offset = align_offset(col_w, text_len, cell.align);

                            for _ in 0..offset {
                                line.push(' ');
                            }
                            line.push_str(cell_text);
                        }
                        CellContent::Divider(style) => {
                            // Only draw divider on the last row
                            if row == max_lines - 1 {
                                let ch = divider_char(*style);
                                for _ in 0..col_w {
                                    line.push(ch);
                                }
                            }
                        }
                    }
                }

                out.push_str(line.trim_end());
                out.push('\n');
            }
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

        Node::Feed { amount, unit } => {
            // Text can only do whole lines; round fractional, drop mm
            let lines = match unit {
                FeedUnit::Lines => amount.round() as usize,
                FeedUnit::Mm => 0, // sub-line precision not possible in plain text
            };
            for _ in 0..lines {
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
