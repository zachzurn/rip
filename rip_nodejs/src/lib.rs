//! Native Node.js addon for Rip receipt markup rendering.
//!
//! All render functions are async (return Promise) and run on libuv's
//! thread pool via `AsyncTask`, so they never block the Node.js event loop.
//!
//! ## Usage patterns
//!
//! **Simple (local resources only):**
//! ```js
//! const png = await renderImage("## Hello", { resourceDir: "./assets" });
//! ```
//!
//! **With remote URLs (host fetches):**
//! ```js
//! const doc = parse(markup);
//! const needed = resolveResources(doc, { resourceDir, cacheDir });
//! const resources = {};
//! for (const url of needed) {
//!     resources[url] = Buffer.from(await fetch(url).then(r => r.arrayBuffer()));
//! }
//! const png = await renderImage(doc, { resourceDir, cacheDir, resources });
//! ```

use std::collections::HashMap;
use std::path::PathBuf;

use napi::bindgen_prelude::*;
use napi::Task;
use napi_derive::napi;

// ─── Document ────────────────────────────────────────────────────────────────

/// Parsed receipt markup document.
///
/// Created by `parse()`. Pass to `resolveResources()` and `render*()` functions
/// to avoid re-parsing the markup.
#[napi]
pub struct Document {
    nodes: Vec<rip::Node>,
}

/// Parse receipt markup into a Document.
///
/// The document can be passed to `resolveResources()` and all `render*()`
/// functions. Parsing once and rendering multiple times is more efficient
/// than passing a markup string to each render call.
#[napi]
pub fn parse(source: String) -> Document {
    Document {
        nodes: rip::parse(&source),
    }
}

// ─── Config ─────────────────────────────────────────────────────────────────

/// Configuration for resource loading and caching.
///
/// All fields are optional. Provide `resourceDir` for local file paths,
/// `cacheDir` for persistent caching, and `resources` for pre-fetched
/// remote content (from `resolveResources()`).
#[napi(object)]
pub struct RenderConfig {
    /// Base directory for resolving relative resource paths (fonts, images).
    pub resource_dir: Option<String>,
    /// Directory for caching downloaded and processed resources.
    pub cache_dir: Option<String>,
    /// Pre-fetched remote resource bytes, keyed by URL.
    ///
    /// Use `resolveResources()` to discover which URLs need fetching,
    /// then populate this map before calling render functions.
    pub resources: Option<HashMap<String, Buffer>>,
}

fn to_resource_config(config: Option<RenderConfig>) -> rip::ResourceConfig {
    match config {
        Some(c) => {
            let resources = c
                .resources
                .unwrap_or_default()
                .into_iter()
                .map(|(k, v)| (k, v.to_vec()))
                .collect();
            rip::ResourceConfig {
                resource_dir: c.resource_dir.map(PathBuf::from),
                cache_dir: c.cache_dir.map(PathBuf::from),
                resources,
            }
        }
        None => rip::ResourceConfig::default(),
    }
}

// ─── resolveResources ───────────────────────────────────────────────────────

/// Discover which remote URLs need to be fetched by the host.
///
/// Returns an array of HTTPS URLs that are not in the download cache.
/// Fetch these with `fetch()` or your HTTP client, then pass the bytes
/// in `config.resources` when rendering.
///
/// Returns an empty array if all resources are local or already cached.
#[napi]
pub fn resolve_resources(doc: &Document, config: Option<RenderConfig>) -> Vec<String> {
    let rc = to_resource_config(config);
    rip::resolve_resources(&doc.nodes, &rc)
}

// ─── renderImage ────────────────────────────────────────────────────────────

pub struct RenderImageTask {
    nodes: Vec<rip::Node>,
    config: rip::ResourceConfig,
}

#[napi]
impl Task for RenderImageTask {
    type Output = Vec<u8>;
    type JsValue = Buffer;

    fn compute(&mut self) -> Result<Self::Output> {
        rip::render_image(&self.nodes, &self.config)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.into())
    }
}

/// Render a Document to a 1-bit black/white PNG image.
///
/// Returns a Buffer containing the compressed PNG bytes.
/// Matches thermal printer appearance.
#[napi(ts_return_type = "Promise<Buffer>")]
pub fn render_image(doc: &Document, config: Option<RenderConfig>) -> AsyncTask<RenderImageTask> {
    AsyncTask::new(RenderImageTask {
        nodes: doc.nodes.clone(),
        config: to_resource_config(config),
    })
}

/// Render markup string to a 1-bit black/white PNG image.
///
/// Convenience function that parses and renders in one step.
/// For repeated rendering or remote resources, use `parse()` + `renderImage()`.
#[napi(ts_return_type = "Promise<Buffer>")]
pub fn render_image_from_markup(source: String, config: Option<RenderConfig>) -> AsyncTask<RenderImageTask> {
    AsyncTask::new(RenderImageTask {
        nodes: rip::parse(&source),
        config: to_resource_config(config),
    })
}

// ─── renderRaster ───────────────────────────────────────────────────────────

pub struct RenderRasterTask {
    nodes: Vec<rip::Node>,
    config: rip::ResourceConfig,
}

#[napi]
impl Task for RenderRasterTask {
    type Output = Vec<u8>;
    type JsValue = Buffer;

    fn compute(&mut self) -> Result<Self::Output> {
        rip::render_raster(&self.nodes, &self.config)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.into())
    }
}

/// Render a Document to ESC/POS raster print commands.
///
/// Returns a Buffer containing the complete ESC/POS byte stream
/// (init + raster image + feed). Send directly to a thermal printer.
#[napi(ts_return_type = "Promise<Buffer>")]
pub fn render_raster(doc: &Document, config: Option<RenderConfig>) -> AsyncTask<RenderRasterTask> {
    AsyncTask::new(RenderRasterTask {
        nodes: doc.nodes.clone(),
        config: to_resource_config(config),
    })
}

/// Render markup string to ESC/POS raster print commands.
#[napi(ts_return_type = "Promise<Buffer>")]
pub fn render_raster_from_markup(source: String, config: Option<RenderConfig>) -> AsyncTask<RenderRasterTask> {
    AsyncTask::new(RenderRasterTask {
        nodes: rip::parse(&source),
        config: to_resource_config(config),
    })
}

// ─── renderEscpos ───────────────────────────────────────────────────────────

pub struct RenderEscposTask {
    nodes: Vec<rip::Node>,
    config: rip::ResourceConfig,
}

#[napi]
impl Task for RenderEscposTask {
    type Output = Vec<u8>;
    type JsValue = Buffer;

    fn compute(&mut self) -> Result<Self::Output> {
        Ok(rip::render_escpos(&self.nodes, &self.config))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.into())
    }
}

/// Render a Document to ESC/POS binary commands using the printer's built-in text engine.
///
/// Returns a Buffer of raw ESC/POS bytes. Images are sent inline as raster data.
#[napi(ts_return_type = "Promise<Buffer>")]
pub fn render_escpos(doc: &Document, config: Option<RenderConfig>) -> AsyncTask<RenderEscposTask> {
    AsyncTask::new(RenderEscposTask {
        nodes: doc.nodes.clone(),
        config: to_resource_config(config),
    })
}

/// Render markup string to ESC/POS binary commands.
#[napi(ts_return_type = "Promise<Buffer>")]
pub fn render_escpos_from_markup(source: String, config: Option<RenderConfig>) -> AsyncTask<RenderEscposTask> {
    AsyncTask::new(RenderEscposTask {
        nodes: rip::parse(&source),
        config: to_resource_config(config),
    })
}

// ─── renderHtml ─────────────────────────────────────────────────────────────

pub struct RenderHtmlTask {
    nodes: Vec<rip::Node>,
}

#[napi]
impl Task for RenderHtmlTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> Result<Self::Output> {
        Ok(rip::render_html(&self.nodes))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

/// Render a Document to a standalone HTML document.
///
/// No resources needed — images are `<img>` tags, QR codes and barcodes are inline SVG.
#[napi(ts_return_type = "Promise<string>")]
pub fn render_html(doc: &Document) -> AsyncTask<RenderHtmlTask> {
    AsyncTask::new(RenderHtmlTask {
        nodes: doc.nodes.clone(),
    })
}

/// Render markup string to a standalone HTML document.
#[napi(ts_return_type = "Promise<string>")]
pub fn render_html_from_markup(source: String) -> AsyncTask<RenderHtmlTask> {
    AsyncTask::new(RenderHtmlTask {
        nodes: rip::parse(&source),
    })
}

// ─── renderText ─────────────────────────────────────────────────────────────

pub struct RenderTextTask {
    nodes: Vec<rip::Node>,
}

#[napi]
impl Task for RenderTextTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> Result<Self::Output> {
        Ok(rip::render_text(&self.nodes))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

/// Render a Document to plain text (monospace ASCII).
///
/// Images, QR codes, and barcodes are rendered as text placeholders.
#[napi(ts_return_type = "Promise<string>")]
pub fn render_text(doc: &Document) -> AsyncTask<RenderTextTask> {
    AsyncTask::new(RenderTextTask {
        nodes: doc.nodes.clone(),
    })
}

/// Render markup string to plain text.
#[napi(ts_return_type = "Promise<string>")]
pub fn render_text_from_markup(source: String) -> AsyncTask<RenderTextTask> {
    AsyncTask::new(RenderTextTask {
        nodes: rip::parse(&source),
    })
}
