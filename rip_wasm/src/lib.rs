//! WASM bindings for Rip receipt markup rendering.
//!
//! Provides free functions exported via `wasm-bindgen` that wrap the
//! `rip` unified API. The JS wrapper (`rip.mjs`) handles WASM init,
//! resource fetching, and image decoding — users just call static
//! async methods on the `Rip` class.

use wasm_bindgen::prelude::*;
use rip_parser::{parse, collect_resources, RenderResources, ImageData};

// ─── Resource discovery ─────────────────────────────────────────────

/// Parse markup and return the external resource URLs it references.
///
/// Returns `{ fonts: string[], images: string[] }`.
#[wasm_bindgen]
pub fn get_resources(source: &str) -> JsValue {
    let nodes = parse(source);
    let urls = collect_resources(&nodes);

    let obj = js_sys::Object::new();

    let fonts = js_sys::Array::new();
    for f in &urls.fonts {
        fonts.push(&JsValue::from_str(f));
    }

    let images = js_sys::Array::new();
    for i in &urls.images {
        images.push(&JsValue::from_str(i));
    }

    js_sys::Reflect::set(&obj, &"fonts".into(), &fonts).unwrap();
    js_sys::Reflect::set(&obj, &"images".into(), &images).unwrap();

    obj.into()
}

// ─── Renderers ──────────────────────────────────────────────────────

/// Render markup to a standalone HTML document.
///
/// No resources needed — images are `<img>` tags, QR/barcodes are inline SVG.
#[wasm_bindgen]
pub fn render_html(source: &str) -> String {
    let nodes = parse(source);
    rip::render_html(&nodes)
}

/// Render markup to plain text (monospace ASCII).
#[wasm_bindgen]
pub fn render_text(source: &str) -> String {
    let nodes = parse(source);
    rip::render_text(&nodes)
}

/// Render markup to 8-bit grayscale pixels (anti-aliased).
///
/// Returns `{ width, height, pixels: Uint8Array, dirtyRows: boolean[] }`.
#[wasm_bindgen]
pub fn render_pixels(source: &str, resources_js: JsValue) -> Result<JsValue, JsValue> {
    let nodes = parse(source);
    let resources = js_to_resources(&resources_js);
    let output = rip::render_luma8(&nodes, &resources)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(pixel_output_to_js(&output))
}

/// Render markup to 1-bit packed pixels (thresholded black/white).
///
/// Returns `{ width, height, pixels: Uint8Array, dirtyRows: boolean[] }`.
/// Pixels are MSB-first packed, `ceil(width/8)` bytes per row.
#[wasm_bindgen]
pub fn render_raster(source: &str, resources_js: JsValue) -> Result<JsValue, JsValue> {
    let nodes = parse(source);
    let resources = js_to_resources(&resources_js);
    let output = rip::render_luma1(&nodes, &resources)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(pixel_output_to_js(&output))
}

/// Render markup to ESC/POS binary commands for thermal printers.
///
/// Returns a `Uint8Array` of raw ESC/POS bytes.
#[wasm_bindgen]
pub fn render_escpos(source: &str, resources_js: JsValue) -> JsValue {
    let nodes = parse(source);
    let resources = js_to_resources(&resources_js);
    let bytes = rip::render_escpos(&nodes, &resources);
    js_sys::Uint8Array::from(&bytes[..]).into()
}

// ─── JS ↔ Rust conversion helpers ──────────────────────────────────

/// Convert a `rip::PixelOutput` to a JS object.
fn pixel_output_to_js(output: &rip::PixelOutput) -> JsValue {
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &"width".into(), &JsValue::from(output.width)).unwrap();
    js_sys::Reflect::set(&obj, &"height".into(), &JsValue::from(output.height)).unwrap();

    let pixels = js_sys::Uint8Array::from(&output.pixels[..]);
    js_sys::Reflect::set(&obj, &"pixels".into(), &pixels).unwrap();

    let dirty_rows = js_sys::Array::new();
    for &dirty in &output.dirty_rows {
        dirty_rows.push(&JsValue::from_bool(dirty));
    }
    js_sys::Reflect::set(&obj, &"dirtyRows".into(), &dirty_rows).unwrap();

    obj.into()
}

/// Convert a JS resources object to `RenderResources`.
///
/// Expected shape:
/// ```js
/// {
///   images: { "url": { width: number, height: number, pixels: Uint8Array } },
///   fonts:  { "url": Uint8Array }
/// }
/// ```
fn js_to_resources(val: &JsValue) -> RenderResources {
    let mut resources = RenderResources::default();

    if val.is_undefined() || val.is_null() {
        return resources;
    }

    // Read images
    if let Ok(images_obj) = js_sys::Reflect::get(val, &"images".into()) {
        if !images_obj.is_undefined() && !images_obj.is_null() {
            let obj: &js_sys::Object = images_obj.unchecked_ref();
            let entries = js_sys::Object::entries(obj);
            for i in 0..entries.length() {
                let entry_val = entries.get(i);
                let entry: &js_sys::Array = entry_val.unchecked_ref();
                if let Some(key) = entry.get(0).as_string() {
                    let img_obj = entry.get(1);
                    let width = js_sys::Reflect::get(&img_obj, &"width".into())
                        .ok()
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0) as u32;
                    let height = js_sys::Reflect::get(&img_obj, &"height".into())
                        .ok()
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0) as u32;
                    if let Ok(px_val) = js_sys::Reflect::get(&img_obj, &"pixels".into()) {
                        let typed_arr: &js_sys::Uint8Array = px_val.unchecked_ref();
                        resources.images.insert(
                            key,
                            ImageData {
                                width,
                                height,
                                pixels: typed_arr.to_vec(),
                            },
                        );
                    }
                }
            }
        }
    }

    // Read fonts
    if let Ok(fonts_obj) = js_sys::Reflect::get(val, &"fonts".into()) {
        if !fonts_obj.is_undefined() && !fonts_obj.is_null() {
            let obj: &js_sys::Object = fonts_obj.unchecked_ref();
            let entries = js_sys::Object::entries(obj);
            for i in 0..entries.length() {
                let entry_val = entries.get(i);
                let entry: &js_sys::Array = entry_val.unchecked_ref();
                if let Some(key) = entry.get(0).as_string() {
                    let bytes_val = entry.get(1);
                    let typed_arr: &js_sys::Uint8Array = bytes_val.unchecked_ref();
                    resources.fonts.insert(key, typed_arr.to_vec());
                }
            }
        }
    }

    resources
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_html_basic() {
        let html = render_html("## Hello");
        assert!(html.contains("Hello"));
    }

    #[test]
    fn render_text_basic() {
        let text = render_text("## Hello");
        assert!(text.contains("Hello"));
    }
}
