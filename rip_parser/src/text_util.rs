//! Shared text utilities for monospace renderers (plain text, ESC/POS).

use crate::ast::{Align, CellContent, LineStyle, Node, Span};

/// Concatenate span texts, stripping all style information.
pub fn spans_to_text(spans: &[Span]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

/// Center text within the given width, padding with spaces.
pub fn center(text: &str, width: usize) -> String {
    if text.len() >= width {
        return text[..width].to_string();
    }
    let pad = (width - text.len()) / 2;
    let mut s = String::with_capacity(width);
    for _ in 0..pad {
        s.push(' ');
    }
    s.push_str(text);
    s
}

/// Word-wrap text to fit within `max_width` characters.
pub fn word_wrap(text: &str, max_width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    if text.len() <= max_width {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            if word.len() > max_width {
                let mut remaining = word;
                while remaining.len() > max_width {
                    lines.push(remaining[..max_width].to_string());
                    remaining = &remaining[max_width..];
                }
                current = remaining.to_string();
            } else {
                current = word.to_string();
            }
        } else if current.len() + 1 + word.len() <= max_width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            if word.len() > max_width {
                let mut remaining = word;
                while remaining.len() > max_width {
                    lines.push(remaining[..max_width].to_string());
                    remaining = &remaining[max_width..];
                }
                current = remaining.to_string();
            } else {
                current = word.to_string();
            }
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        vec![String::new()]
    } else {
        lines
    }
}

/// Compute alignment offset (in characters) within a cell.
pub fn align_offset(cell_width: usize, content_width: usize, align: Align) -> usize {
    if content_width >= cell_width {
        return 0;
    }
    match align {
        Align::Left => 0,
        Align::Right => cell_width - content_width,
        Align::Center => (cell_width - content_width) / 2,
    }
}

/// A contiguous group of column rows with the same number of cells.
///
/// All rows in the group share the same computed column widths,
/// so the widest content in each column position dictates proportions.
pub struct ColumnGroup {
    /// Start index into the `&[Node]` slice (inclusive).
    pub start: usize,
    /// End index into the `&[Node]` slice (exclusive).
    pub end: usize,
    /// Number of cells per row in this group.
    pub col_count: usize,
}

/// Identify contiguous groups of `Node::Columns` with the same column count.
///
/// Any non-column node breaks a group. Column rows with different cell
/// counts also start a new group.
pub fn identify_column_groups(nodes: &[Node]) -> Vec<ColumnGroup> {
    let mut groups = Vec::new();
    let mut i = 0;
    while i < nodes.len() {
        if let Node::Columns { cells, .. } = &nodes[i] {
            let col_count = cells.len();
            let start = i;
            i += 1;
            while i < nodes.len() {
                if let Node::Columns { cells: next_cells, .. } = &nodes[i] {
                    if next_cells.len() == col_count {
                        i += 1;
                        continue;
                    }
                }
                break;
            }
            groups.push(ColumnGroup { start, end: i, col_count });
        } else {
            i += 1;
        }
    }
    groups
}

/// Extract width percentages from a column group.
///
/// Scans all rows in the group and takes the first non-None `width_pct`
/// found for each column position.  Only considers rows whose explicit
/// percentages sum to ≤ 100 (rows with invalid sums are skipped entirely).
pub fn group_width_pcts(nodes: &[Node], group: &ColumnGroup) -> Vec<Option<u32>> {
    let mut result = vec![None; group.col_count];
    for idx in group.start..group.end {
        if let Node::Columns { cells, .. } = &nodes[idx] {
            // Skip rows whose explicit percentages sum > 100
            let row_sum: u32 = cells.iter().filter_map(|c| c.width_pct).sum();
            if row_sum > 100 {
                continue;
            }
            for (col, cell) in cells.iter().enumerate() {
                if col < result.len() && result[col].is_none() {
                    result[col] = cell.width_pct;
                }
            }
        }
    }
    result
}

/// Compute column widths with content-aware auto-fit.
///
/// - Cells with explicit `width_pct` get their specified share of available width.
/// - Auto cells (no `width_pct`) share remaining space proportionally by `natural_widths`.
/// - If all natural widths are zero, auto cells split equally.
/// - If explicit percentages sum > 100, all are ignored (everything becomes auto).
/// - A `gap` is placed between adjacent columns (not at edges).
/// - Each column gets at least `min_col` width.
///
/// Returns `(x, width)` pairs where x accounts for gaps between columns.
pub fn column_widths(
    width_pcts: &[Option<u32>],
    natural_widths: &[f64],
    total_width: f64,
    gap: f64,
    min_col: f64,
) -> Vec<(f64, f64)> {
    let n = width_pcts.len();
    if n == 0 {
        return vec![];
    }

    let total_gaps = gap * (n as f64 - 1.0).max(0.0);
    let available = (total_width - total_gaps).max(min_col * n as f64);

    // Sum explicit percentages
    let explicit_sum: u32 = width_pcts.iter().filter_map(|p| *p).sum();

    // If percentages sum > 100, ignore all
    let pcts: Vec<Option<u32>> = if explicit_sum > 100 {
        vec![None; n]
    } else {
        width_pcts.to_vec()
    };

    // Compute widths for explicit-percentage columns
    let mut widths = vec![0.0f64; n];
    let mut remaining = available;

    for (i, pct) in pcts.iter().enumerate() {
        if let Some(p) = pct {
            let w = (*p as f64 / 100.0 * available).max(min_col);
            widths[i] = w;
            remaining -= w;
        }
    }
    remaining = remaining.max(0.0);

    // Distribute remaining space to auto columns proportionally by natural widths
    let auto_indices: Vec<usize> = pcts.iter().enumerate()
        .filter(|(_, p)| p.is_none())
        .map(|(i, _)| i)
        .collect();

    if !auto_indices.is_empty() {
        let auto_natural_sum: f64 = auto_indices.iter()
            .map(|&i| natural_widths.get(i).copied().unwrap_or(0.0))
            .sum();

        if auto_natural_sum > 0.0 {
            if auto_natural_sum <= remaining {
                // Everything fits — give each column its natural width.
                for &i in &auto_indices {
                    let nat = natural_widths.get(i).copied().unwrap_or(0.0);
                    widths[i] = nat.max(min_col);
                }
                // Compute leftover after min_col clamping to avoid overflow
                let assigned: f64 = auto_indices.iter().map(|&i| widths[i]).sum();
                let leftover = (remaining - assigned).max(0.0);

                // Distribute leftover: if columns vary in width, give it
                // all to the widest (receipt pattern). If all equal, split evenly.
                let max_nat = auto_indices.iter()
                    .map(|&i| natural_widths.get(i).copied().unwrap_or(0.0))
                    .fold(0.0f64, f64::max);
                let min_nat = auto_indices.iter()
                    .map(|&i| natural_widths.get(i).copied().unwrap_or(0.0))
                    .fold(f64::MAX, f64::min);

                if max_nat - min_nat < 0.001 {
                    // All equal — distribute evenly
                    let share = leftover / auto_indices.len() as f64;
                    for &i in &auto_indices {
                        widths[i] += share;
                    }
                } else {
                    // Give leftover to the widest column
                    let widest_idx = auto_indices.iter()
                        .copied()
                        .max_by(|&a, &b| {
                            let na = natural_widths.get(a).copied().unwrap_or(0.0);
                            let nb = natural_widths.get(b).copied().unwrap_or(0.0);
                            na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
                        });
                    if let Some(wi) = widest_idx {
                        widths[wi] += leftover;
                    }
                }
            } else {
                // Content overflows — greedy water-fill: give small columns
                // their full natural width first, squeeze only the large ones.
                let mut sorted: Vec<usize> = auto_indices.clone();
                sorted.sort_by(|&a, &b| {
                    let na = natural_widths.get(a).copied().unwrap_or(0.0);
                    let nb = natural_widths.get(b).copied().unwrap_or(0.0);
                    na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
                });

                let mut budget = remaining;
                let mut unfilled = sorted.len() as f64;

                for &i in &sorted {
                    let nat = natural_widths.get(i).copied().unwrap_or(0.0);
                    let fair_share = budget / unfilled;
                    let w = nat.min(fair_share).max(min_col);
                    widths[i] = w;
                    budget -= w;
                    unfilled -= 1.0;
                }
            }
        } else {
            // All natural widths zero (e.g. all dividers) — equal split
            let w = remaining / auto_indices.len() as f64;
            for &i in &auto_indices {
                widths[i] = w.max(min_col);
            }
        }
    }

    // Build (x, width) pairs with gaps
    let mut result = Vec::with_capacity(n);
    let mut x = 0.0;
    for (i, w) in widths.iter().enumerate() {
        result.push((x, *w));
        x += w;
        if i < n - 1 {
            x += gap;
        }
    }
    result
}

/// Map a LineStyle to its divider character.
pub fn divider_char(style: LineStyle) -> char {
    match style {
        LineStyle::Thin => '-',
        LineStyle::Thick => '=',
        LineStyle::Dotted => '.',
    }
}

/// Measure natural content widths (in characters) for a column group.
///
/// Returns the max character count per column position across all rows
/// in the group. Useful for text/escpos renderers.
pub fn measure_natural_widths_chars(nodes: &[Node], group: &ColumnGroup) -> Vec<f64> {
    let mut max_natural = vec![0.0f64; group.col_count];
    for idx in group.start..group.end {
        if let Node::Columns { cells, .. } = &nodes[idx] {
            for (col, cell) in cells.iter().enumerate() {
                if col < max_natural.len() {
                    if let CellContent::Spans(spans) = &cell.content {
                        let text = spans_to_text(spans);
                        max_natural[col] = max_natural[col].max(text.len() as f64);
                    }
                }
            }
        }
    }
    max_natural
}

/// Pre-compute column widths for all groups in a character-based renderer.
///
/// Returns a map from node index to pre-computed `(x, width)` pairs.
/// Used by rip_text and rip_escpos.
pub fn precompute_columns_chars(
    nodes: &[Node],
    total_width: usize,
    gap: f64,
    min_col: f64,
) -> std::collections::HashMap<usize, Vec<(f64, f64)>> {
    let groups = identify_column_groups(nodes);
    let mut cache = std::collections::HashMap::new();

    for group in &groups {
        let pcts = group_width_pcts(nodes, group);
        let natural = measure_natural_widths_chars(nodes, group);
        let cols = column_widths(&pcts, &natural, total_width as f64, gap, min_col);

        for idx in group.start..group.end {
            cache.insert(idx, cols.clone());
        }
    }

    cache
}
