//! Integration tests that run inside a real WASM environment (Node.js via wasm-pack).

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_node_experimental);

// ─── render_html ────────────────────────────────────────────────────

#[wasm_bindgen_test]
fn html_basic_text() {
    let html = rip_wasm::render_html("Hello World");
    assert!(html.contains("Hello World"), "HTML should contain the text");
    assert!(html.contains("<html"), "Should be a full HTML document");
}

#[wasm_bindgen_test]
fn html_title() {
    let html = rip_wasm::render_html("## Receipt");
    assert!(html.contains("Receipt"));
}

#[wasm_bindgen_test]
fn html_columns() {
    let html = rip_wasm::render_html("| Item | $5.00 |");
    assert!(html.contains("Item"));
    assert!(html.contains("$5.00"));
}

#[wasm_bindgen_test]
fn html_divider() {
    let html = rip_wasm::render_html("---");
    assert!(!html.is_empty());
}

#[wasm_bindgen_test]
fn html_full_receipt() {
    let markup = "\
## My Store
---
| Coffee | $4.50 |
| Bagel | $3.00 |
===
| Total | $7.50 |

Thank you!";
    let html = rip_wasm::render_html(markup);
    assert!(html.contains("My Store"));
    assert!(html.contains("Coffee"));
    assert!(html.contains("$7.50"));
    assert!(html.contains("Thank you!"));
}

// ─── render_text ────────────────────────────────────────────────────

#[wasm_bindgen_test]
fn text_basic() {
    let text = rip_wasm::render_text("Hello World");
    assert!(text.contains("Hello World"));
}

#[wasm_bindgen_test]
fn text_columns() {
    let text = rip_wasm::render_text("| Left | Right |");
    assert!(text.contains("Left"));
    assert!(text.contains("Right"));
}

// ─── get_resources ──────────────────────────────────────────────────

#[wasm_bindgen_test]
fn resources_empty_for_plain_text() {
    let val = rip_wasm::get_resources("Hello World");
    let fonts = js_sys::Reflect::get(&val, &"fonts".into()).unwrap();
    let images = js_sys::Reflect::get(&val, &"images".into()).unwrap();
    let fonts_arr: &js_sys::Array = fonts.unchecked_ref();
    let images_arr: &js_sys::Array = images.unchecked_ref();
    assert_eq!(fonts_arr.length(), 0);
    assert_eq!(images_arr.length(), 0);
}

#[wasm_bindgen_test]
fn resources_detects_image() {
    let val = rip_wasm::get_resources("@image(logo.png, 200)");
    let images = js_sys::Reflect::get(&val, &"images".into()).unwrap();
    let images_arr: &js_sys::Array = images.unchecked_ref();
    assert_eq!(images_arr.length(), 1);
    assert_eq!(images_arr.get(0).as_string().unwrap(), "logo.png");
}

#[wasm_bindgen_test]
fn resources_detects_font() {
    let val = rip_wasm::get_resources("@style(text, receipt.ttf, 12)");
    let fonts = js_sys::Reflect::get(&val, &"fonts".into()).unwrap();
    let fonts_arr: &js_sys::Array = fonts.unchecked_ref();
    assert_eq!(fonts_arr.length(), 1);
    assert_eq!(fonts_arr.get(0).as_string().unwrap(), "receipt.ttf");
}

// ─── render_pixels ──────────────────────────────────────────────────

#[wasm_bindgen_test]
fn pixels_basic() {
    let resources = JsValue::null();
    let result = rip_wasm::render_pixels("Hello", resources);
    assert!(result.is_ok(), "render_pixels should succeed");

    let val = result.unwrap();
    let width = js_sys::Reflect::get(&val, &"width".into())
        .unwrap()
        .as_f64()
        .unwrap();
    let height = js_sys::Reflect::get(&val, &"height".into())
        .unwrap()
        .as_f64()
        .unwrap();
    let pixels = js_sys::Reflect::get(&val, &"pixels".into()).unwrap();
    let dirty_rows = js_sys::Reflect::get(&val, &"dirtyRows".into()).unwrap();

    assert!(width > 0.0, "width should be positive");
    assert!(height > 0.0, "height should be positive");
    assert!(pixels.is_instance_of::<js_sys::Uint8Array>(), "pixels should be Uint8Array");
    assert!(dirty_rows.is_instance_of::<js_sys::Array>(), "dirtyRows should be Array");

    let pixels_arr: &js_sys::Uint8Array = pixels.unchecked_ref();
    assert_eq!(
        pixels_arr.length(),
        (width * height) as u32,
        "pixel count should equal width × height"
    );
}

#[wasm_bindgen_test]
fn pixels_empty_document_errors() {
    let resources = JsValue::null();
    let result = rip_wasm::render_pixels("// just a comment", resources);
    assert!(result.is_err(), "empty document should return an error");
}

// ─── render_raster ──────────────────────────────────────────────────

#[wasm_bindgen_test]
fn raster_basic() {
    let resources = JsValue::null();
    let result = rip_wasm::render_raster("Hello", resources);
    assert!(result.is_ok(), "render_raster should succeed");

    let val = result.unwrap();
    let width = js_sys::Reflect::get(&val, &"width".into())
        .unwrap()
        .as_f64()
        .unwrap();
    let height = js_sys::Reflect::get(&val, &"height".into())
        .unwrap()
        .as_f64()
        .unwrap();
    let pixels = js_sys::Reflect::get(&val, &"pixels".into()).unwrap();

    assert!(width > 0.0);
    assert!(height > 0.0);

    // 1-bit packed: ceil(width/8) bytes per row
    let pixels_arr: &js_sys::Uint8Array = pixels.unchecked_ref();
    let expected_len = ((width as u32 + 7) / 8) * height as u32;
    assert_eq!(pixels_arr.length(), expected_len, "packed pixel length should match");
}

// ─── render_escpos ──────────────────────────────────────────────────

#[wasm_bindgen_test]
fn escpos_basic() {
    let resources = JsValue::null();
    let result = rip_wasm::render_escpos("Hello", resources);
    assert!(result.is_instance_of::<js_sys::Uint8Array>(), "should return Uint8Array");

    let arr: &js_sys::Uint8Array = result.unchecked_ref();
    assert!(arr.length() > 0, "ESC/POS output should not be empty");

    // Should start with ESC @ (initialize printer)
    let bytes = arr.to_vec();
    assert_eq!(bytes[0], 0x1B, "first byte should be ESC");
    assert_eq!(bytes[1], 0x40, "second byte should be @ (init)");
}

#[wasm_bindgen_test]
fn escpos_cut_command() {
    let resources = JsValue::null();
    let result = rip_wasm::render_escpos("Hello\n@cut()", resources);
    let arr: &js_sys::Uint8Array = result.unchecked_ref();
    let bytes = arr.to_vec();

    // Should contain GS V (cut command): 0x1D 0x56
    let has_cut = bytes.windows(2).any(|w| w[0] == 0x1D && w[1] == 0x56);
    assert!(has_cut, "should contain cut command");
}

// ─── Full round-trip ────────────────────────────────────────────────

#[wasm_bindgen_test]
fn all_renderers_agree_on_content() {
    let markup = "## Hello\n---\n| Item | $5.00 |";
    let resources = JsValue::null();

    let html = rip_wasm::render_html(markup);
    let text = rip_wasm::render_text(markup);
    let pixels = rip_wasm::render_pixels(markup, resources.clone());
    let raster = rip_wasm::render_raster(markup, resources.clone());
    let escpos = rip_wasm::render_escpos(markup, resources);

    // All should produce non-empty output
    assert!(!html.is_empty());
    assert!(!text.is_empty());
    assert!(pixels.is_ok());
    assert!(raster.is_ok());
    assert!(!escpos.is_falsy());

    // HTML and text should contain the key content
    assert!(html.contains("Hello"));
    assert!(html.contains("Item"));
    assert!(html.contains("$5.00"));
    assert!(text.contains("Hello"));
    assert!(text.contains("Item"));
    assert!(text.contains("$5.00"));
}
