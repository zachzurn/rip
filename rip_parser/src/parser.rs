use crate::ast::*;
use crate::inline::parse_spans;

/// Parse a complete Rip document into a flat list of AST nodes.
pub fn parse(input: &str) -> Vec<Node> {
    input
        .lines()
        .filter_map(|line| parse_line(line, Size::Text))
        .collect()
}

/// Classify and parse a single line. Returns `None` for comments.
fn parse_line(line: &str, size: Size) -> Option<Node> {
    // Empty line → blank
    if line.trim().is_empty() {
        return Some(Node::Blank);
    }

    let trimmed = line.trim();

    // Comment
    if trimmed.starts_with("//") {
        return None;
    }

    // Escaped first character → plain text
    if trimmed.starts_with('\\') {
        return Some(Node::Text {
            spans: parse_spans(trimmed),
            size,
        });
    }

    // Directive
    if trimmed.starts_with('@') {
        return parse_directive(trimmed, None);
    }

    // Size-wrapped lines (check longest match first)
    if let Some(node) = try_parse_size_wrapped(trimmed) {
        return Some(node);
    }

    // Dividers (3+ repeated characters)
    if let Some(style) = try_parse_divider(trimmed) {
        return Some(Node::Line { style });
    }

    // Pipe line (must start and end with |)
    if trimmed.starts_with('|') && trimmed.ends_with('|') {
        return Some(parse_pipe_line(trimmed, size));
    }

    // Plain text (default)
    Some(Node::Text {
        spans: parse_spans(trimmed),
        size,
    })
}

// ---------------------------------------------------------------------------
// Size-wrapped lines
// ---------------------------------------------------------------------------

/// Try to parse a size-wrapped line. Returns the inner node with the correct size.
fn try_parse_size_wrapped(line: &str) -> Option<Node> {
    // Check headers: #### → ### → ## (longest match first)
    if let Some(inner) = strip_balanced_marker(line, "####") {
        return parse_line(inner, Size::TitleL);
    }
    if let Some(inner) = strip_balanced_marker(line, "###") {
        return parse_line(inner, Size::TitleM);
    }
    if let Some(inner) = strip_balanced_marker(line, "##") {
        return parse_line(inner, Size::Title);
    }

    // Check body sizes: +++ → ++ (longest match first)
    if let Some(inner) = strip_balanced_marker(line, "+++") {
        return parse_line(inner, Size::TextL);
    }
    if let Some(inner) = strip_balanced_marker(line, "++") {
        return parse_line(inner, Size::TextM);
    }

    None
}

/// Strip matching markers from both sides of a line.
/// The line must start with the marker. If a closing marker exists, strip it too.
/// Returns the trimmed inner content.
fn strip_balanced_marker<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    if !line.starts_with(marker) {
        return None;
    }

    let after_open = &line[marker.len()..];

    // Check for closing marker
    let inner = if after_open.trim_end().ends_with(marker) {
        let end = after_open.trim_end();
        &end[..end.len() - marker.len()]
    } else {
        after_open
    };

    Some(inner.trim())
}

// ---------------------------------------------------------------------------
// Dividers
// ---------------------------------------------------------------------------

/// Check if a line is a divider (3+ of the same character: `-`, `=`, `.`).
fn try_parse_divider(line: &str) -> Option<LineStyle> {
    let trimmed = line.trim();
    if trimmed.len() < 3 {
        return None;
    }

    let first = trimmed.chars().next()?;
    let style = match first {
        '-' => LineStyle::Thin,
        '=' => LineStyle::Thick,
        '.' => LineStyle::Dotted,
        _ => return None,
    };

    if trimmed.chars().all(|c| c == first) {
        Some(style)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Pipe lines (columns / alignment)
// ---------------------------------------------------------------------------

/// Parse a pipe-delimited line into a Columns node (or a directive with alignment).
fn parse_pipe_line(line: &str, size: Size) -> Node {
    let cells = split_pipe_cells(line);
    let cell_count = cells.len();

    // Single cell containing only a directive → promote to directive with alignment
    if cell_count == 1 {
        let (content, align) = &cells[0];
        let trimmed = content.trim();
        if trimmed.starts_with('@') {
            if let Some(node) = parse_directive(trimmed, *align) {
                return node;
            }
        }
    }

    // Build column cells with auto-alignment
    let columns: Vec<Cell> = cells
        .into_iter()
        .enumerate()
        .map(|(i, (content, explicit_align))| {
            let align = match explicit_align {
                Some(a) => a,
                None => auto_align(i, cell_count),
            };

            // Check if cell content is a divider
            let trimmed = content.trim();
            if let Some(line_style) = try_parse_divider(trimmed) {
                Cell {
                    content: CellContent::Divider(line_style),
                    align,
                }
            } else {
                Cell {
                    content: CellContent::Spans(parse_spans(trimmed)),
                    align,
                }
            }
        })
        .collect();

    Node::Columns {
        cells: columns,
        size,
    }
}

/// Split pipe-delimited content into cells, extracting per-cell alignment.
/// Returns `None` alignment when no explicit `>` or `<` markers are present.
///
/// Input: `| Item Name |> $8.99 |`
/// Output: `[("Item Name", None), ("$8.99", Some(Right))]`
fn split_pipe_cells(line: &str) -> Vec<(String, Option<Align>)> {
    let mut cells = Vec::new();
    let content = line.trim();
    if !content.starts_with('|') || !content.ends_with('|') {
        return cells;
    }

    // Remove outer pipes and split
    // We need to be careful about escaped pipes: \|
    let segments = split_on_pipes(content);

    // segments[0] is empty (before first |), segments[last] is empty (after last |)
    // The real cells are segments[1..segments.len()-1]
    if segments.len() < 2 {
        return cells;
    }

    for i in 1..segments.len() - 1 {
        let raw = &segments[i];
        let (cell_content, align) = extract_cell_alignment(raw);
        cells.push((cell_content, align));
    }

    cells
}

/// Split a string on unescaped `|` characters.
fn split_on_pipes(input: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            current.push(ch);
            continue;
        }
        if ch == '|' {
            segments.push(current);
            current = String::new();
        } else {
            current.push(ch);
        }
    }
    segments.push(current);
    segments
}

/// Extract alignment from a cell's raw content.
///
/// The `>` or `<` modifiers appear at the edges of the cell content
/// (right after the opening pipe or right before the closing pipe).
///
/// `> text` → Right
/// `> text <` → Center
/// `text` → Left (default)
fn extract_cell_alignment(raw: &str) -> (String, Option<Align>) {
    let trimmed = raw.trim();

    let starts_with_gt = trimmed.starts_with('>');
    let ends_with_lt = trimmed.ends_with('<');

    if starts_with_gt && ends_with_lt {
        // |> text <| → Center
        let inner = &trimmed[1..trimmed.len() - 1];
        (inner.trim().to_string(), Some(Align::Center))
    } else if starts_with_gt {
        // |> text | → Right
        let inner = &trimmed[1..];
        (inner.trim().to_string(), Some(Align::Right))
    } else if ends_with_lt {
        // | text <| → explicitly marked left
        let inner = &trimmed[..trimmed.len() - 1];
        (inner.trim().to_string(), Some(Align::Left))
    } else {
        // | text | → no explicit markers
        (trimmed.to_string(), None)
    }
}

/// Determine auto-alignment based on column position and total count.
///
/// - 2 columns: left, right
/// - 3 columns: left, center, right
/// - Other: left for all
fn auto_align(index: usize, count: usize) -> Align {
    match count {
        2 => {
            if index == 0 {
                Align::Left
            } else {
                Align::Right
            }
        }
        3 => match index {
            0 => Align::Left,
            1 => Align::Center,
            2 => Align::Right,
            _ => Align::Left,
        },
        _ => Align::Left,
    }
}

// ---------------------------------------------------------------------------
// Directives
// ---------------------------------------------------------------------------

/// Parse a `@name(args)` directive line.
fn parse_directive(line: &str, align: Option<Align>) -> Option<Node> {
    let trimmed = line.trim();
    if !trimmed.starts_with('@') {
        return None;
    }

    // Find the name and args
    let after_at = &trimmed[1..];
    let paren_start = after_at.find('(')?;
    let name = after_at[..paren_start].trim();

    // Find matching closing paren
    let paren_end = after_at.rfind(')')?;
    let args_str = &after_at[paren_start + 1..paren_end];
    let args: Vec<&str> = if args_str.trim().is_empty() {
        Vec::new()
    } else {
        args_str.split(',').map(|a| a.trim()).collect()
    };

    match name {
        "printer-width" => {
            let mm = parse_length_to_mm(args.first()?)?;
            Some(Node::PrinterWidth { mm })
        }
        "printer-dpi" => {
            let dpi = args.first()?.parse::<u32>().ok()?;
            Some(Node::PrinterDpi { dpi })
        }
        "style" => {
            let level = parse_size_level(args.first()?)?;
            let font = args.get(1)?.to_string();
            let points = args.get(2)?.parse::<f64>().ok()?;
            Some(Node::Style {
                level,
                font,
                points,
            })
        }
        "image" => {
            let url = args.first()?.to_string();
            let width = args.get(1).and_then(|s| s.parse::<f64>().ok());
            let height = args.get(2).and_then(|s| s.parse::<f64>().ok());
            Some(Node::Image {
                url,
                width,
                height,
                align,
            })
        }
        "qr" => {
            let data = args.first()?.to_string();
            let size = args.get(1).and_then(|s| s.parse::<f64>().ok());
            Some(Node::Qr { data, size, align })
        }
        "barcode" => {
            let format = args.first()?.to_string();
            let data = args.get(1)?.to_string();
            Some(Node::Barcode { format, data })
        }
        "cut" => {
            let partial = args.first().map_or(false, |s| *s == "partial");
            Some(Node::Cut { partial })
        }
        "feed" => {
            let lines = args.first()?.parse::<u32>().ok()?;
            Some(Node::Feed { lines })
        }
        "drawer" => Some(Node::Drawer),
        _ => None,
    }
}

/// Parse a length value with unit suffix into millimeters.
///
/// Supports: `"3in"` → 76.2, `"80mm"` → 80.0, `"8cm"` → 80.0.
/// If no suffix, assumes millimeters.
fn parse_length_to_mm(s: &str) -> Option<f64> {
    let s = s.trim();
    if let Some(num) = s.strip_suffix("in") {
        num.trim().parse::<f64>().ok().map(|v| v * 25.4)
    } else if let Some(num) = s.strip_suffix("mm") {
        num.trim().parse::<f64>().ok()
    } else if let Some(num) = s.strip_suffix("cm") {
        num.trim().parse::<f64>().ok().map(|v| v * 10.0)
    } else {
        // No suffix: assume mm for backward compatibility
        s.parse::<f64>().ok()
    }
}

/// Parse a size level name to its enum value.
fn parse_size_level(name: &str) -> Option<Size> {
    match name {
        "text" => Some(Size::Text),
        "text-m" => Some(Size::TextM),
        "text-l" => Some(Size::TextL),
        "title" => Some(Size::Title),
        "title-m" => Some(Size::TitleM),
        "title-l" => Some(Size::TitleL),
        _ => None,
    }
}
