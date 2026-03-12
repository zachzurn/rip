use std::fs;
use std::path::Path;

use rip_image::{render_pixels, RenderError};
use rip_parser::{parse, collect_resources};
use rip_resources::{ImageData, RenderResources};

/// Create a synthetic grayscale test image (checkerboard pattern).
fn synthetic_image(width: u32, height: u32) -> ImageData {
    let mut pixels = vec![255u8; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            if (x / 4 + y / 4) % 2 == 0 {
                pixels[(y * width + x) as usize] = 0;
            }
        }
    }
    ImageData { width, height, pixels }
}

/// Helper: parse source and render to pixels, loading fonts from
/// fixtures and providing synthetic images for any referenced URLs.
fn render(source: &str) -> Result<(u32, u32, Vec<u8>), RenderError> {
    let fixtures = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../examples/resources"));
    let nodes = parse(source);
    let res = collect_resources(&nodes);

    let mut resources = RenderResources::default();

    for url in &res.fonts {
        let path = if Path::new(url).is_absolute() {
            Path::new(url).to_path_buf()
        } else {
            fixtures.join(url)
        };
        if let Ok(bytes) = fs::read(&path) {
            resources.fonts.insert(url.clone(), bytes);
        }
    }

    for url in &res.images {
        resources.images.insert(url.clone(), synthetic_image(100, 80));
    }

    render_pixels(&nodes, &resources)
}

// ── Basic rendering ─────────────────────────────────────────────────────

#[test]
fn plain_text_renders() {
    let (w, h, pixels) = render("Hello, world!").unwrap();
    assert!(w > 0);
    assert!(h > 0);
    assert_eq!(pixels.len(), (w * h) as usize);
}

#[test]
fn empty_document_is_error() {
    let result = render("// only comments");
    assert!(result.is_err());
}

#[test]
fn blank_lines_have_height() {
    let (_, h1, _) = render("one line").unwrap();
    let (_, h2, _) = render("one line\n\n\n").unwrap();
    assert!(h2 > h1, "blank lines should add height");
}

// ── Text styles ─────────────────────────────────────────────────────────

#[test]
fn bold_text_renders() {
    let (w, h, pixels) = render("*bold text*").unwrap();
    assert!(w > 0 && h > 0);
    assert_eq!(pixels.len(), (w * h) as usize);
}

#[test]
fn italic_text_renders() {
    let (w, h, pixels) = render("~italic text~").unwrap();
    assert!(w > 0 && h > 0);
    assert_eq!(pixels.len(), (w * h) as usize);
}

#[test]
fn underline_text_renders() {
    let (w, h, pixels) = render("_underline_").unwrap();
    assert!(w > 0 && h > 0);
    assert_eq!(pixels.len(), (w * h) as usize);
}

#[test]
fn mixed_styles_render() {
    let (_, _, pixels) = render("normal *bold* _under_ ~italic~ -strike-").unwrap();
    assert!(!pixels.is_empty());
}

// ── Sizes ───────────────────────────────────────────────────────────────

#[test]
fn title_is_taller_than_text_with_styles() {
    // Without @style directives, all sizes fall back to the default font.
    // Provide explicit styles to test size differentiation.
    let source_text = "@style(text, default, 10)\nplain text";
    let source_title = "@style(title, default, 20)\n## title text";
    let (_, h_text, _) = render(source_text).unwrap();
    let (_, h_title, _) = render(source_title).unwrap();
    assert!(
        h_title > h_text,
        "title ({h_title}) should be taller than text ({h_text})"
    );
}

#[test]
fn larger_sizes_are_taller_with_styles() {
    // Use wide size gaps and explicit DPI to ensure measurable differences.
    let source1 = "@printer-dpi(203)\n@style(text, default, 10)\nsmall text";
    let source2 = "@printer-dpi(203)\n@style(text, default, 24)\nmedium text";
    let source3 = "@printer-dpi(203)\n@style(text, default, 48)\nlarge text";
    let (_, h1, _) = render(source1).unwrap();
    let (_, h2, _) = render(source2).unwrap();
    let (_, h3, _) = render(source3).unwrap();
    assert!(h2 > h1, "24pt ({h2}) > 10pt ({h1})");
    assert!(h3 > h2, "48pt ({h3}) > 24pt ({h2})");
}

// ── Columns ─────────────────────────────────────────────────────────────

#[test]
fn two_column_layout() {
    let (w, h, pixels) = render("Left | Right").unwrap();
    assert!(w > 0 && h > 0);
    assert_eq!(pixels.len(), (w * h) as usize);
}

#[test]
fn three_column_layout() {
    let (_, _, pixels) = render("A | B | C").unwrap();
    assert!(!pixels.is_empty());
}

#[test]
fn column_with_divider() {
    let (_, _, pixels) = render("Left | --- | Right").unwrap();
    assert!(!pixels.is_empty());
}

// ── Dividers ────────────────────────────────────────────────────────────

#[test]
fn thin_divider() {
    let (w, h, pixels) = render("---").unwrap();
    assert!(w > 0 && h > 0);
    // Divider should contain some dark pixels (foreground)
    assert!(pixels.iter().any(|&p| p == 0), "thin divider should have dark pixels");
}

#[test]
fn thick_divider() {
    let (_, h_thin, _) = render("---").unwrap();
    let (_, h_thick, _) = render("===").unwrap();
    assert!(
        h_thick >= h_thin,
        "thick divider ({h_thick}) should be >= thin ({h_thin})"
    );
}

#[test]
fn dotted_divider() {
    let (_, _, pixels) = render("...").unwrap();
    assert!(pixels.iter().any(|&p| p == 0), "dotted divider should have dark pixels");
}

// ── Images ──────────────────────────────────────────────────────────────

#[test]
fn local_image_renders() {
    let (w, h, pixels) = render("@image(receipt.png)").unwrap();
    assert!(w > 0 && h > 0);
    assert_eq!(pixels.len(), (w * h) as usize);
}

#[test]
fn image_with_dimensions() {
    let (_, _, pixels) = render("@image(receipt.png, 50, 50)").unwrap();
    assert!(!pixels.is_empty());
}

// ── QR codes ────────────────────────────────────────────────────────────

#[test]
fn qr_code_renders() {
    let (w, h, pixels) = render("@qr(https://example.com)").unwrap();
    assert!(w > 0 && h > 0);
    // QR should contain dark modules
    assert!(pixels.iter().any(|&p| p == 0), "QR code should have dark pixels");
}

#[test]
fn qr_code_with_size() {
    let (_, h_default, _) = render("@qr(test)").unwrap();
    let (_, h_small, _) = render("@qr(test, 50)").unwrap();
    assert!(
        h_default > h_small,
        "default QR ({h_default}) should be taller than 50pt QR ({h_small})"
    );
}

// ── Barcodes ────────────────────────────────────────────────────────────

#[test]
fn barcode_code128() {
    let (_, _, pixels) = render("@barcode(CODE128, ABC-123)").unwrap();
    assert!(pixels.iter().any(|&p| p == 0), "barcode should have dark bars");
}

#[test]
fn barcode_ean13() {
    let (_, _, pixels) = render("@barcode(EAN13, 4006381333931)").unwrap();
    assert!(!pixels.is_empty());
}

// ── Feed ────────────────────────────────────────────────────────────────

#[test]
fn feed_adds_height() {
    let (_, h1, _) = render("text").unwrap();
    let (_, h2, _) = render("text\n@feed(5)").unwrap();
    assert!(h2 > h1, "feed should add height");
}

// ── Rip configuration ───────────────────────────────────────────────────

#[test]
fn printer_width_affects_output() {
    let (w1, _, _) = render("@printer-width(80)\ntext").unwrap();
    let (w2, _, _) = render("@printer-width(58)\ntext").unwrap();
    assert!(
        w1 > w2,
        "80mm paper ({w1}px) should be wider than 58mm ({w2}px)"
    );
}

#[test]
fn printer_dpi_affects_output() {
    let (w1, _, _) = render("@printer-dpi(203)\ntext").unwrap();
    let (w2, _, _) = render("@printer-dpi(300)\ntext").unwrap();
    assert!(
        w2 > w1,
        "300dpi ({w2}px) should be wider than 203dpi ({w1}px) at same mm width"
    );
}

// ── Full document ───────────────────────────────────────────────────────

#[test]
fn full_receipt_renders() {
    let source = "\
@printer-width(80)

## RECEIPT

---

Item A | $10.00
Item B | $5.50
...
Subtotal | $15.50
Tax | $1.24
===
# *TOTAL* | *$16.74*

---

@qr(https://example.com/receipt/1234)

Thank you!
@feed(3)
@cut
";
    let (w, h, pixels) = render(source).unwrap();
    assert!(w > 0 && h > 0);
    assert_eq!(pixels.len(), (w * h) as usize);
    // Should have a mix of light (background=255) and dark (foreground=0) pixels
    assert!(pixels.iter().any(|&p| p == 0));
    assert!(pixels.iter().any(|&p| p == 255));
}
