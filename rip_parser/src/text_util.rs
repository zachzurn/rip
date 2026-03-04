//! Shared text utilities for monospace renderers (plain text, ESC/POS).

use crate::ast::{Align, LineStyle, Span};

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

/// Map a LineStyle to its divider character.
pub fn divider_char(style: LineStyle) -> char {
    match style {
        LineStyle::Thin => '-',
        LineStyle::Thick => '=',
        LineStyle::Dotted => '.',
    }
}
