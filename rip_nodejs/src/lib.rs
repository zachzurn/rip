//! Native Node.js addon for Rip receipt markup rendering.
//!
//! All render functions are async (return Promise) and run on libuv's
//! thread pool via `AsyncTask`, so they never block the Node.js event loop.
//!
//! Resource fetching, image decoding, caching — all handled by Rust internally.
//! No JS-side resource management needed.

use std::path::PathBuf;

use napi::bindgen_prelude::*;
use napi::Task;
use napi_derive::napi;

// ─── Config ─────────────────────────────────────────────────────────────────

/// Configuration for resource loading and caching.
///
/// Both fields are optional. Even with no paths, HTTPS URLs can still be fetched.
#[napi(object)]
pub struct RenderConfig {
    /// Base directory for resolving relative resource paths (fonts, images).
    pub resource_dir: Option<String>,
    /// Directory for caching downloaded and processed resources.
    pub cache_dir: Option<String>,
}

fn to_resource_config(config: Option<RenderConfig>) -> rip::ResourceConfig {
    match config {
        Some(c) => rip::ResourceConfig {
            resource_dir: c.resource_dir.map(PathBuf::from),
            cache_dir: c.cache_dir.map(PathBuf::from),
        },
        None => rip::ResourceConfig::default(),
    }
}

// ─── renderImage ────────────────────────────────────────────────────────────

pub struct RenderImageTask {
    source: String,
    config: rip::ResourceConfig,
}

#[napi]
impl Task for RenderImageTask {
    type Output = Vec<u8>;
    type JsValue = Buffer;

    fn compute(&mut self) -> Result<Self::Output> {
        let nodes = rip::parse(&self.source);
        rip::render_image(&nodes, &self.config)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.into())
    }
}

/// Render markup to a 1-bit black/white PNG image.
///
/// Returns a Buffer containing the compressed PNG bytes.
/// Matches thermal printer appearance.
#[napi(ts_return_type = "Promise<Buffer>")]
pub fn render_image(source: String, config: Option<RenderConfig>) -> AsyncTask<RenderImageTask> {
    AsyncTask::new(RenderImageTask {
        source,
        config: to_resource_config(config),
    })
}

// ─── renderRaster ───────────────────────────────────────────────────────────

pub struct RenderRasterTask {
    source: String,
    config: rip::ResourceConfig,
}

#[napi]
impl Task for RenderRasterTask {
    type Output = Vec<u8>;
    type JsValue = Buffer;

    fn compute(&mut self) -> Result<Self::Output> {
        let nodes = rip::parse(&self.source);
        rip::render_raster(&nodes, &self.config)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.into())
    }
}

/// Render markup to ESC/POS raster print commands.
///
/// Returns a Buffer containing the complete ESC/POS byte stream
/// (init + raster image + feed). Send directly to a thermal printer.
#[napi(ts_return_type = "Promise<Buffer>")]
pub fn render_raster(source: String, config: Option<RenderConfig>) -> AsyncTask<RenderRasterTask> {
    AsyncTask::new(RenderRasterTask {
        source,
        config: to_resource_config(config),
    })
}

// ─── renderEscpos ───────────────────────────────────────────────────────────

pub struct RenderEscposTask {
    source: String,
    config: rip::ResourceConfig,
}

#[napi]
impl Task for RenderEscposTask {
    type Output = Vec<u8>;
    type JsValue = Buffer;

    fn compute(&mut self) -> Result<Self::Output> {
        let nodes = rip::parse(&self.source);
        Ok(rip::render_escpos(&nodes, &self.config))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.into())
    }
}

/// Render markup to ESC/POS binary commands using the printer's built-in text engine.
///
/// Returns a Buffer of raw ESC/POS bytes. Images are sent inline as raster data.
#[napi(ts_return_type = "Promise<Buffer>")]
pub fn render_escpos(source: String, config: Option<RenderConfig>) -> AsyncTask<RenderEscposTask> {
    AsyncTask::new(RenderEscposTask {
        source,
        config: to_resource_config(config),
    })
}

// ─── renderHtml ─────────────────────────────────────────────────────────────

pub struct RenderHtmlTask {
    source: String,
}

#[napi]
impl Task for RenderHtmlTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> Result<Self::Output> {
        let nodes = rip::parse(&self.source);
        Ok(rip::render_html(&nodes))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

/// Render markup to a standalone HTML document.
///
/// No resources needed — images are `<img>` tags, QR codes and barcodes are inline SVG.
#[napi(ts_return_type = "Promise<string>")]
pub fn render_html(source: String) -> AsyncTask<RenderHtmlTask> {
    AsyncTask::new(RenderHtmlTask { source })
}

// ─── renderText ─────────────────────────────────────────────────────────────

pub struct RenderTextTask {
    source: String,
}

#[napi]
impl Task for RenderTextTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> Result<Self::Output> {
        let nodes = rip::parse(&self.source);
        Ok(rip::render_text(&nodes))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

/// Render markup to plain text (monospace ASCII).
///
/// Images, QR codes, and barcodes are rendered as text placeholders.
#[napi(ts_return_type = "Promise<string>")]
pub fn render_text(source: String) -> AsyncTask<RenderTextTask> {
    AsyncTask::new(RenderTextTask { source })
}
