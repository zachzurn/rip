use pretty_assertions::assert_eq;
use rip_parser::ast::*;
use rip_parser::parse;

fn span(text: &str, style: SpanStyle) -> Span {
    Span {
        text: text.to_string(),
        style,
    }
}

fn normal(text: &str) -> Span {
    span(text, SpanStyle::Normal)
}

// ── Blank lines ──────────────────────────────────────────────────────────

#[test]
fn blank_line() {
    let nodes = parse("\n");
    assert_eq!(nodes, vec![Node::Blank]);
}

#[test]
fn multiple_blanks() {
    let nodes = parse("\n\n\n");
    assert_eq!(nodes, vec![Node::Blank, Node::Blank, Node::Blank]);
}

// ── Comments ─────────────────────────────────────────────────────────────

#[test]
fn comment_skipped() {
    let nodes = parse("// this is a comment");
    assert_eq!(nodes, vec![]);
}

#[test]
fn comment_with_leading_whitespace() {
    let nodes = parse("  // indented comment");
    assert_eq!(nodes, vec![]);
}

// ── Plain text ───────────────────────────────────────────────────────────

#[test]
fn plain_text() {
    let nodes = parse("hello world");
    assert_eq!(
        nodes,
        vec![Node::Text {
            spans: vec![normal("hello world")],
            size: Size::Text,
        }]
    );
}

#[test]
fn text_with_bold() {
    let nodes = parse("*bold text*");
    assert_eq!(
        nodes,
        vec![Node::Text {
            spans: vec![span("bold text", SpanStyle::Bold)],
            size: Size::Text,
        }]
    );
}

#[test]
fn text_with_mixed_styles() {
    let nodes = parse("before *bold* after");
    assert_eq!(
        nodes,
        vec![Node::Text {
            spans: vec![
                normal("before "),
                span("bold", SpanStyle::Bold),
                normal(" after"),
            ],
            size: Size::Text,
        }]
    );
}

// ── Dividers ─────────────────────────────────────────────────────────────

#[test]
fn thin_divider() {
    let nodes = parse("---");
    assert_eq!(nodes, vec![Node::Line { style: LineStyle::Thin }]);
}

#[test]
fn thick_divider() {
    let nodes = parse("===");
    assert_eq!(nodes, vec![Node::Line { style: LineStyle::Thick }]);
}

#[test]
fn dotted_divider() {
    let nodes = parse("...");
    assert_eq!(nodes, vec![Node::Line { style: LineStyle::Dotted }]);
}

#[test]
fn long_divider() {
    let nodes = parse("----------");
    assert_eq!(nodes, vec![Node::Line { style: LineStyle::Thin }]);
}

#[test]
fn two_dashes_not_divider() {
    let nodes = parse("--");
    assert_eq!(
        nodes,
        vec![Node::Text {
            spans: vec![normal("--")],
            size: Size::Text,
        }]
    );
}

// ── Size-wrapped lines ──────────────────────────────────────────────────

#[test]
fn header_title() {
    let nodes = parse("## small header ##");
    assert_eq!(
        nodes,
        vec![Node::Text {
            spans: vec![normal("small header")],
            size: Size::Title,
        }]
    );
}

#[test]
fn header_title_m() {
    let nodes = parse("### medium header ###");
    assert_eq!(
        nodes,
        vec![Node::Text {
            spans: vec![normal("medium header")],
            size: Size::TitleM,
        }]
    );
}

#[test]
fn header_title_l() {
    let nodes = parse("#### BURGER BARN ####");
    assert_eq!(
        nodes,
        vec![Node::Text {
            spans: vec![normal("BURGER BARN")],
            size: Size::TitleL,
        }]
    );
}

#[test]
fn body_text_m() {
    let nodes = parse("++ larger text ++");
    assert_eq!(
        nodes,
        vec![Node::Text {
            spans: vec![normal("larger text")],
            size: Size::TextM,
        }]
    );
}

#[test]
fn body_text_l() {
    let nodes = parse("+++ largest text +++");
    assert_eq!(
        nodes,
        vec![Node::Text {
            spans: vec![normal("largest text")],
            size: Size::TextL,
        }]
    );
}

#[test]
fn size_wrapped_without_closing_marker() {
    // Start/end rule: no closing marker, rest is content
    let nodes = parse("### header without close");
    assert_eq!(
        nodes,
        vec![Node::Text {
            spans: vec![normal("header without close")],
            size: Size::TitleM,
        }]
    );
}

#[test]
fn size_wrapped_pipe_line() {
    let nodes = parse("++ | *TOTAL* |> *$19.74* | ++");
    assert_eq!(
        nodes,
        vec![Node::Columns {
            cells: vec![
                Cell {
                    content: CellContent::Spans(vec![span("TOTAL", SpanStyle::Bold)]),
                    align: Align::Left,
                },
                Cell {
                    content: CellContent::Spans(vec![span("$19.74", SpanStyle::Bold)]),
                    align: Align::Right,
                },
            ],
            size: Size::TextM,
        }]
    );
}

// ── Pipe lines (columns / alignment) ────────────────────────────────────

#[test]
fn two_column_auto_align() {
    let nodes = parse("| Item Name |> $8.99 |");
    assert_eq!(
        nodes,
        vec![Node::Columns {
            cells: vec![
                Cell {
                    content: CellContent::Spans(vec![normal("Item Name")]),
                    align: Align::Left,
                },
                Cell {
                    content: CellContent::Spans(vec![normal("$8.99")]),
                    align: Align::Right,
                },
            ],
            size: Size::Text,
        }]
    );
}

#[test]
fn centered_text() {
    let nodes = parse("|> centered text <|");
    assert_eq!(
        nodes,
        vec![Node::Columns {
            cells: vec![Cell {
                content: CellContent::Spans(vec![normal("centered text")]),
                align: Align::Center,
            }],
            size: Size::Text,
        }]
    );
}

#[test]
fn right_aligned_text() {
    let nodes = parse("|> right aligned |");
    assert_eq!(
        nodes,
        vec![Node::Columns {
            cells: vec![Cell {
                content: CellContent::Spans(vec![normal("right aligned")]),
                align: Align::Right,
            }],
            size: Size::Text,
        }]
    );
}

#[test]
fn left_aligned_text() {
    let nodes = parse("| left aligned |");
    assert_eq!(
        nodes,
        vec![Node::Columns {
            cells: vec![Cell {
                content: CellContent::Spans(vec![normal("left aligned")]),
                align: Align::Left,
            }],
            size: Size::Text,
        }]
    );
}

#[test]
fn pipe_without_closing_is_plain_text() {
    let nodes = parse("| no closing pipe");
    assert_eq!(
        nodes,
        vec![Node::Text {
            spans: vec![normal("| no closing pipe")],
            size: Size::Text,
        }]
    );
}

// ── Directives ──────────────────────────────────────────────────────────

#[test]
fn printer_width_mm() {
    let nodes = parse("@printer-width(80mm)");
    assert_eq!(nodes, vec![Node::PrinterWidth { mm: 80.0 }]);
}

#[test]
fn printer_width_inches() {
    let nodes = parse("@printer-width(3in)");
    match &nodes[0] {
        Node::PrinterWidth { mm } => {
            assert!((mm - 76.2).abs() < 0.001, "expected ~76.2, got {mm}");
        }
        other => panic!("expected PrinterWidth, got {other:?}"),
    }
}

#[test]
fn printer_width_cm() {
    let nodes = parse("@printer-width(8cm)");
    assert_eq!(nodes, vec![Node::PrinterWidth { mm: 80.0 }]);
}

#[test]
fn printer_width_no_suffix() {
    let nodes = parse("@printer-width(80)");
    assert_eq!(nodes, vec![Node::PrinterWidth { mm: 80.0 }]);
}

#[test]
fn printer_dpi_directive() {
    let nodes = parse("@printer-dpi(203)");
    assert_eq!(nodes, vec![Node::PrinterDpi { dpi: 203 }]);
}

#[test]
fn printer_threshold_directive() {
    let nodes = parse("@printer-threshold(160)");
    assert_eq!(nodes, vec![Node::PrinterThreshold { threshold: 160 }]);
}

#[test]
fn style_directive() {
    let nodes = parse("@style(text, https://fonts.example.com/Mono.ttf, 12)");
    assert_eq!(
        nodes,
        vec![Node::Style {
            level: Size::Text,
            font: "https://fonts.example.com/Mono.ttf".to_string(),
            points: 12.0,
        }]
    );
}

#[test]
fn style_default_font() {
    let nodes = parse("@style(title, default, 24)");
    assert_eq!(
        nodes,
        vec![Node::Style {
            level: Size::Title,
            font: "default".to_string(),
            points: 24.0,
        }]
    );
}

#[test]
fn image_directive() {
    let nodes = parse("@image(https://store.com/logo.png)");
    assert_eq!(
        nodes,
        vec![Node::Image {
            url: "https://store.com/logo.png".to_string(),
            width: None,
            height: None,
            align: None,
        }]
    );
}

#[test]
fn image_with_width() {
    let nodes = parse("@image(https://store.com/logo.png, 200)");
    assert_eq!(
        nodes,
        vec![Node::Image {
            url: "https://store.com/logo.png".to_string(),
            width: Some(200.0),
            height: None,
            align: None,
        }]
    );
}

#[test]
fn image_with_width_and_height() {
    let nodes = parse("@image(https://store.com/logo.png, 200, 100)");
    assert_eq!(
        nodes,
        vec![Node::Image {
            url: "https://store.com/logo.png".to_string(),
            width: Some(200.0),
            height: Some(100.0),
            align: None,
        }]
    );
}

#[test]
fn image_centered_in_pipes() {
    let nodes = parse("|> @image(https://store.com/logo.png, 200) <|");
    assert_eq!(
        nodes,
        vec![Node::Image {
            url: "https://store.com/logo.png".to_string(),
            width: Some(200.0),
            height: None,
            align: Some(Align::Center),
        }]
    );
}

#[test]
fn qr_directive() {
    let nodes = parse("@qr(https://example.com)");
    assert_eq!(
        nodes,
        vec![Node::Qr {
            data: "https://example.com".to_string(),
            size: None,
            align: None,
        }]
    );
}

#[test]
fn qr_with_size() {
    let nodes = parse("@qr(https://example.com, 200)");
    assert_eq!(
        nodes,
        vec![Node::Qr {
            data: "https://example.com".to_string(),
            size: Some(200.0),
            align: None,
        }]
    );
}

#[test]
fn qr_centered_in_pipes() {
    let nodes = parse("|> @qr(https://example.com) <|");
    assert_eq!(
        nodes,
        vec![Node::Qr {
            data: "https://example.com".to_string(),
            size: None,
            align: Some(Align::Center),
        }]
    );
}

#[test]
fn barcode_directive() {
    let nodes = parse("@barcode(CODE128, 1234567890)");
    assert_eq!(
        nodes,
        vec![Node::Barcode {
            format: "CODE128".to_string(),
            data: "1234567890".to_string(),
        }]
    );
}

#[test]
fn cut_full() {
    let nodes = parse("@cut()");
    assert_eq!(nodes, vec![Node::Cut { partial: false }]);
}

#[test]
fn cut_partial() {
    let nodes = parse("@cut(partial)");
    assert_eq!(nodes, vec![Node::Cut { partial: true }]);
}

#[test]
fn feed_directive() {
    let nodes = parse("@feed(3)");
    assert_eq!(nodes, vec![Node::Feed { lines: 3 }]);
}

#[test]
fn drawer_directive() {
    let nodes = parse("@drawer()");
    assert_eq!(nodes, vec![Node::Drawer]);
}

// ── Escaping ────────────────────────────────────────────────────────────

#[test]
fn escaped_hash_not_header() {
    let nodes = parse("\\## not a header");
    assert_eq!(
        nodes,
        vec![Node::Text {
            spans: vec![normal("## not a header")],
            size: Size::Text,
        }]
    );
}

#[test]
fn escaped_pipe_not_column() {
    let nodes = parse("\\| not a pipe |");
    assert_eq!(
        nodes,
        vec![Node::Text {
            spans: vec![normal("| not a pipe |")],
            size: Size::Text,
        }]
    );
}

// ── Full document ───────────────────────────────────────────────────────

#[test]
fn full_receipt() {
    let input = r#"@printer-width(80mm)
@style(text, https://fonts.example.com/RobotoMono.ttf, 12)

// Store Header
#### BURGER BARN ####
|> 742 Evergreen Terrace <|

===

| Order #1042        |> 02/25/2026 |

---

| Classic Burger     |>  $8.99 |
| Cheese Fries       |>  $4.50 |

...

| Subtotal           |> $18.24 |
===
++ | *TOTAL*         |> *$19.74* | ++

|> @qr(https://burgerbarn.com/receipt/1042) <|

@feed(2)
@cut()
@drawer()"#;

    let nodes = parse(input);

    // Verify key nodes
    assert_eq!(nodes[0], Node::PrinterWidth { mm: 80.0 });
    assert_eq!(
        nodes[1],
        Node::Style {
            level: Size::Text,
            font: "https://fonts.example.com/RobotoMono.ttf".to_string(),
            points: 12.0,
        }
    );
    assert_eq!(nodes[2], Node::Blank);
    // Comment is skipped, so next is BURGER BARN
    assert_eq!(
        nodes[3],
        Node::Text {
            spans: vec![normal("BURGER BARN")],
            size: Size::TitleL,
        }
    );
    assert_eq!(
        nodes[4],
        Node::Columns {
            cells: vec![Cell {
                content: CellContent::Spans(vec![normal("742 Evergreen Terrace")]),
                align: Align::Center,
            }],
            size: Size::Text,
        }
    );
    assert_eq!(nodes[5], Node::Blank);
    assert_eq!(nodes[6], Node::Line { style: LineStyle::Thick });

    // Verify the TOTAL line is size TextM
    let total_node = nodes.iter().find(|n| {
        matches!(n, Node::Columns { size: Size::TextM, .. })
    });
    assert!(total_node.is_some(), "Should have a TextM columns node for TOTAL");

    // Verify QR is centered
    let qr_node = nodes.iter().find(|n| matches!(n, Node::Qr { .. }));
    assert_eq!(
        qr_node,
        Some(&Node::Qr {
            data: "https://burgerbarn.com/receipt/1042".to_string(),
            size: None,
            align: Some(Align::Center),
        })
    );

    // Verify printer commands at end
    let len = nodes.len();
    assert_eq!(nodes[len - 3], Node::Feed { lines: 2 });
    assert_eq!(nodes[len - 2], Node::Cut { partial: false });
    assert_eq!(nodes[len - 1], Node::Drawer);
}
