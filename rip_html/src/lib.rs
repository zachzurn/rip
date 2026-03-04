use rip_parser::ast::*;
use rip_parser::encode;
use std::collections::HashMap;
use std::fmt::Write;

/// Configuration collected from @printer-width and @style directives during rendering.
struct StyleConfig {
    paper_width: Option<f64>,
    fonts: HashMap<String, FontConfig>,
}

struct FontConfig {
    url: String,
    points: f64,
}

impl StyleConfig {
    fn new() -> Self {
        Self {
            paper_width: None,
            fonts: HashMap::new(),
        }
    }

    fn apply_style(&mut self, level: Size, font: &str, points: f64) {
        let key = size_css_class(level);
        self.fonts.insert(
            key.to_string(),
            FontConfig {
                url: font.to_string(),
                points,
            },
        );
    }
}

/// Render a parsed Rip AST to a standalone HTML string.
pub fn render_html(nodes: &[Node]) -> String {
    let mut config = StyleConfig::new();
    let mut body = String::new();

    // First pass: collect @printer-width and @style directives
    for node in nodes {
        match node {
            Node::PrinterWidth { mm } => { config.paper_width = Some(*mm); }
            Node::PrinterDpi { .. } => {}  // not used in HTML
            Node::Style {
                level,
                font,
                points,
            } => config.apply_style(*level, font, *points),
            _ => {}
        }
    }

    // Second pass: render nodes to HTML
    for node in nodes {
        render_node(node, &config, &mut body);
    }

    wrap_document(&config, &body)
}

/// Render a single AST node to HTML, appending to `out`.
fn render_node(node: &Node, _config: &StyleConfig, out: &mut String) {
    match node {
        Node::Text { spans, size } => {
            let class = size_css_class(*size);
            let _ = write!(out, "<div class=\"line {class}\">");
            render_spans(spans, out);
            out.push_str("</div>\n");
        }

        Node::Columns { cells, size } => {
            let class = size_css_class(*size);
            let _ = write!(out, "<div class=\"row {class}\">");
            for cell in cells {
                let align = align_css(cell.align);
                let _ = write!(out, "<span class=\"cell\" style=\"text-align:{align}\">");
                match &cell.content {
                    CellContent::Spans(spans) => render_spans(spans, out),
                    CellContent::Divider(line_style) => {
                        let div_class = match line_style {
                            LineStyle::Thin => "thin",
                            LineStyle::Thick => "thick",
                            LineStyle::Dotted => "dotted",
                        };
                        let _ = write!(out, "<hr class=\"divider {div_class}\" />");
                    }
                }
                out.push_str("</span>");
            }
            out.push_str("</div>\n");
        }

        Node::Line { style } => {
            let class = match style {
                LineStyle::Thin => "thin",
                LineStyle::Thick => "thick",
                LineStyle::Dotted => "dotted",
            };
            let _ = write!(out, "<hr class=\"divider {class}\" />\n");
        }

        Node::Blank => {
            out.push_str("<div class=\"blank\"></div>\n");
        }

        Node::Image {
            url,
            width,
            height,
            align,
        } => {
            let align_class = align.map_or("", |a| match a {
                Align::Left => " align-left",
                Align::Center => " align-center",
                Align::Right => " align-right",
            });
            let mut style = String::new();
            if let Some(w) = width {
                let _ = write!(style, "width:{w}pt;");
            }
            if let Some(h) = height {
                let _ = write!(style, "height:{h}pt;");
            }
            let style_attr = if style.is_empty() {
                String::new()
            } else {
                format!(" style=\"{style}\"")
            };
            let _ = write!(
                out,
                "<div class=\"image{align_class}\"><img src=\"{url}\"{style_attr} /></div>\n"
            );
        }

        Node::Qr { data, size, align } => {
            let align_class = align.map_or("", |a| match a {
                Align::Left => " align-left",
                Align::Center => " align-center",
                Align::Right => " align-right",
            });
            let dim = size.unwrap_or(150.0) as u32;
            let _ = write!(out, "<div class=\"qr{align_class}\">");
            if let Some(grid) = encode::encode_qr(data) {
                out.push_str(&qr_to_svg(&grid, dim));
            }
            out.push_str("</div>\n");
        }

        Node::Barcode { format, data } => {
            let _ = write!(out, "<div class=\"barcode\">");
            if let Some(encoded) = encode::encode_barcode(format, data) {
                out.push_str(&barcode_to_svg(&encoded, 50));
            }
            out.push_str("</div>\n");
        }

        Node::Feed { lines } => {
            for _ in 0..*lines {
                out.push_str("<div class=\"blank\"></div>\n");
            }
        }

        // Directives that don't produce visible output
        Node::PrinterWidth { .. } | Node::PrinterDpi { .. }
        | Node::PrinterThreshold { .. } | Node::Style { .. }
        | Node::Cut { .. } | Node::Drawer => {}
    }
}

/// Render a list of spans to inline HTML.
fn render_spans(spans: &[Span], out: &mut String) {
    for span in spans {
        let text = html_escape(&span.text);
        match span.style {
            SpanStyle::Normal => out.push_str(&text),
            SpanStyle::Bold => {
                let _ = write!(out, "<b>{text}</b>");
            }
            SpanStyle::Underline => {
                let _ = write!(out, "<u>{text}</u>");
            }
            SpanStyle::Italic => {
                let _ = write!(out, "<i>{text}</i>");
            }
            SpanStyle::Strikethrough => {
                let _ = write!(out, "<s>{text}</s>");
            }
        }
    }
}

/// Map a Size to a CSS class name.
fn size_css_class(size: Size) -> &'static str {
    match size {
        Size::Text => "text",
        Size::TextM => "text-m",
        Size::TextL => "text-l",
        Size::Title => "title",
        Size::TitleM => "title-m",
        Size::TitleL => "title-l",
    }
}

/// Map an Align to a CSS text-align value.
fn align_css(align: Align) -> &'static str {
    match align {
        Align::Left => "left",
        Align::Center => "center",
        Align::Right => "right",
    }
}

/// Render a QR module grid to an inline SVG string.
///
/// Adds a 4-module quiet zone (per QR spec) and a white background.
fn qr_to_svg(grid: &encode::QrGrid, size: u32) -> String {
    let w = grid.width;
    let quiet = 4u32; // standard quiet zone
    let total = w + 2 * quiet;

    let mut path = String::new();
    for row in 0..w {
        for col in 0..w {
            if grid.modules[(row * w + col) as usize] {
                let x = col + quiet;
                let y = row + quiet;
                let _ = write!(path, "M{x},{y}h1v1h-1z");
            }
        }
    }

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {total} {total}" width="{size}" height="{size}"><rect width="{total}" height="{total}" fill="#fff"/><path d="{path}" fill="#000"/></svg>"##,
    )
}

/// Render a 1D barcode bar pattern to an inline SVG string.
fn barcode_to_svg(bars: &[u8], height: u32) -> String {
    let w = bars.len();
    let mut path = String::new();
    for (i, &bar) in bars.iter().enumerate() {
        if bar == 1 {
            let _ = write!(path, "M{i},0h1v{height}h-1z");
        }
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {height}" preserveAspectRatio="none"><path d="{path}" fill="#000"/></svg>"##,
    )
}

/// Minimal HTML entity escaping.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Wrap rendered body in a complete HTML document with CSS.
fn wrap_document(config: &StyleConfig, body: &str) -> String {
    let paper_width = config.paper_width.unwrap_or(80.0);
    // Approximate mm to px for screen display (1mm ≈ 3.78px)
    let width_px = paper_width * 3.78;

    let mut font_faces = String::new();
    let mut size_rules = String::new();

    for (class, fc) in &config.fonts {
        let font_family = format!("tab-{class}");

        if fc.url != "default" {
            let _ = write!(
                font_faces,
                r#"
    @font-face {{
      font-family: '{font_family}';
      src: url('{}');
    }}"#,
                fc.url
            );
        }

        let family = if fc.url == "default" {
            "monospace".to_string()
        } else {
            format!("'{font_family}', monospace")
        };

        let _ = write!(
            size_rules,
            r#"
    .{class} {{
      font-family: {family};
      font-size: {:.1}pt;
    }}"#,
            fc.points
        );
    }

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <style>
    * {{ margin: 0; padding: 0; box-sizing: border-box; }}
    {font_faces}
    .tab {{
      width: {width_px:.0}px;
      margin: 20px auto;
      padding: 12px;
      background: #fff;
      font-family: monospace;
      font-size: 12pt;
      color: #000;
      border: 1px solid #ddd;
    }}
    .line {{
      padding: 1px 0;
      white-space: pre-wrap;
    }}
    .row {{
      display: flex;
      align-items: flex-end;
      padding: 1px 0;
      white-space: pre-wrap;
    }}
    .row .cell {{
      flex: 1 0 0%;
      word-wrap: break-word;
      overflow-wrap: break-word;
    }}
    .blank {{
      height: 1em;
    }}
    hr.divider {{
      border: none;
      margin: 4px 0;
    }}
    hr.thin {{
      border-top: 1px solid #000;
    }}
    hr.thick {{
      border-top: 3px solid #000;
    }}
    hr.dotted {{
      border-top: 1px dotted #000;
    }}
    .image {{ padding: 4px 0; }}
    .image img {{ max-width: 100%; }}
    .qr {{
      padding: 0;
    }}
    .qr svg {{
      max-width: 100%;
      height: auto;
    }}
    .barcode {{
      padding: 4px 0;
    }}
    .barcode svg {{
      width: 100%;
      height: 50px;
    }}
    .title, .title-m, .title-l {{ text-align: center; }}
    .align-left {{ text-align: left; }}
    .align-center {{ text-align: center; }}
    .align-right {{ text-align: right; }}
    {size_rules}
  </style>
</head>
<body>
  <div class="tab">
{body}  </div>
</body>
</html>"#
    )
}
