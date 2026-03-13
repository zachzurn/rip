//! Resource processing for Rip markup.
//!
//! Handles local file loading, caching, image decoding, scaling, and
//! dithering. For remote URLs (HTTPS), the host provides raw bytes via
//! [`ResourceConfig::resources`] — this crate does no network I/O.
//!
//! Use [`resolve_resources`] to discover which remote URLs need fetching,
//! then pass the bytes in via `config.resources` before rendering.
//!
//! Used by renderers that deal with pixel data (`rip_image`, `rip_escpos`).

pub mod dither;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use rip_parser::ast::Node;
use rip_parser::BLACK_THRESHOLD;
use sha2::{Digest, Sha256};

// ─── Configuration ──────────────────────────────────────────────────────────

/// Configuration for resource loading and caching.
///
/// Provide a `resource_dir` to resolve relative file paths (fonts, images).
/// Provide a `cache_dir` to cache downloaded and processed resources.
/// Provide `resources` with pre-fetched bytes for remote URLs.
#[derive(Debug, Clone, Default)]
pub struct ResourceConfig {
    /// Base directory for resolving relative resource paths (fonts, images).
    /// If `None`, only absolute paths work for local files.
    pub resource_dir: Option<PathBuf>,
    /// Directory for caching downloaded and processed resources.
    /// If `None`, no caching (re-processes every time).
    pub cache_dir: Option<PathBuf>,
    /// Pre-fetched remote resource bytes, keyed by URL.
    ///
    /// Use [`resolve_resources`] to discover which URLs need fetching,
    /// then populate this map before calling render functions.
    pub resources: HashMap<String, Vec<u8>>,
}

// ─── Resource types ─────────────────────────────────────────────────────────

/// Pre-decoded grayscale image.
///
/// Pixels are row-major luma8 (0 = black, 255 = white).
/// Length of `pixels` must equal `width * height`.
#[derive(Debug, Clone)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl ImageData {
    /// Serialize to a cache-friendly byte format: `[u32 LE width][u32 LE height][pixels]`.
    pub fn to_cache_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8 + self.pixels.len());
        buf.extend_from_slice(&self.width.to_le_bytes());
        buf.extend_from_slice(&self.height.to_le_bytes());
        buf.extend_from_slice(&self.pixels);
        buf
    }

    /// Deserialize from cache bytes produced by [`to_cache_bytes`].
    pub fn from_cache_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }
        let width = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let height = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let expected_len = (width as usize) * (height as usize);
        let pixels = &data[8..];
        if pixels.len() != expected_len {
            return None;
        }
        Some(Self {
            width,
            height,
            pixels: pixels.to_vec(),
        })
    }
}

/// Resolved resources for rendering.
///
/// Contains pre-decoded images and raw font bytes. Renderers fall back
/// to embedded default fonts when a requested font is not present.
#[derive(Debug, Clone, Default)]
pub struct RenderResources {
    /// Pre-decoded grayscale images, keyed by the URL/path from the source.
    pub images: HashMap<String, ImageData>,
    /// Raw TTF/OTF font bytes, keyed by the URL/path from `@style` directives.
    pub fonts: HashMap<String, Vec<u8>>,
}

// ─── Image decoding and scaling ─────────────────────────────────────────────

/// Decode raw image bytes (PNG, JPEG, GIF, BMP, WebP) to grayscale luma8.
///
/// Uses BT.601 luma conversion and alpha-blends against a white background.
pub fn decode_image(bytes: &[u8]) -> Option<ImageData> {
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let mut luma = Vec::with_capacity((w * h) as usize);

    for pixel in rgba.pixels() {
        let [r, g, b, a] = pixel.0;
        // BT.601 luma
        let l = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
        // Blend against white background using alpha
        let af = a as f32 / 255.0;
        let blended = l * af + 255.0 * (1.0 - af);
        luma.push(blended.round() as u8);
    }

    Some(ImageData {
        width: w,
        height: h,
        pixels: luma,
    })
}

/// Decode an SVG file to grayscale luma8 at the specified target dimensions.
///
/// SVGs are rendered at full target resolution (no nearest-neighbor artifacts).
/// Returns `None` if the SVG cannot be parsed or rendered.
pub fn decode_svg(bytes: &[u8], target_w: u32, target_h: u32) -> Option<ImageData> {
    let opts = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(bytes, &opts).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(target_w, target_h)?;

    // Fill with white background
    pixmap.fill(resvg::tiny_skia::Color::WHITE);

    // Compute transform to fit SVG into target dimensions
    let svg_size = tree.size();
    let scale_x = target_w as f32 / svg_size.width();
    let scale_y = target_h as f32 / svg_size.height();
    let scale = scale_x.min(scale_y);
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Convert RGBA to luma8 (same as raster images)
    let rgba_data = pixmap.data();
    let pixel_count = (target_w * target_h) as usize;
    let mut luma = Vec::with_capacity(pixel_count);
    for i in 0..pixel_count {
        let offset = i * 4;
        let r = rgba_data[offset];
        let g = rgba_data[offset + 1];
        let b = rgba_data[offset + 2];
        // Alpha already composited against white by pixmap.fill + resvg::render
        let l = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
        luma.push(l.round() as u8);
    }

    Some(ImageData {
        width: target_w,
        height: target_h,
        pixels: luma,
    })
}

/// Nearest-neighbor scale a grayscale pixel buffer.
pub fn scale_nn(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let mut out = vec![255u8; (dst_w * dst_h) as usize];
    for dy in 0..dst_h {
        let sy = (dy as u64 * src_h as u64 / dst_h as u64) as usize;
        for dx in 0..dst_w {
            let sx = (dx as u64 * src_w as u64 / dst_w as u64) as usize;
            out[(dy * dst_w + dx) as usize] = src[sy * src_w as usize + sx];
        }
    }
    out
}

/// Compute scaled (width, height) for an image given its native dimensions
/// and optional maximum bounds, preserving aspect ratio.
///
/// - If both `max_w` and `max_h` are given, fits within both.
/// - If only one is given, scales to that bound without upscaling.
/// - If neither is given, uses `paper_width` as the max width.
pub fn scale_image_dims(
    nat_w: u32,
    nat_h: u32,
    max_w: Option<u32>,
    max_h: Option<u32>,
    paper_width: u32,
) -> (u32, u32) {
    let (w, h) = match (max_w, max_h) {
        (Some(mw), Some(mh)) => {
            let scale = (mw as f32 / nat_w as f32).min(mh as f32 / nat_h as f32);
            ((nat_w as f32 * scale) as u32, (nat_h as f32 * scale) as u32)
        }
        (Some(mw), None) => {
            let scale = (mw as f32 / nat_w as f32).min(1.0);
            ((nat_w as f32 * scale) as u32, (nat_h as f32 * scale) as u32)
        }
        (None, Some(mh)) => {
            let scale = (mh as f32 / nat_h as f32).min(1.0);
            ((nat_w as f32 * scale) as u32, (nat_h as f32 * scale) as u32)
        }
        (None, None) => {
            let mw = paper_width;
            let mh = paper_width;
            let scale = (mw as f32 / nat_w as f32).min(mh as f32 / nat_h as f32);
            ((nat_w as f32 * scale) as u32, (nat_h as f32 * scale) as u32)
        }
    };
    (w.max(1), h.max(1))
}

// ─── Private helpers ────────────────────────────────────────────────────────

/// Check if raw bytes look like an SVG (starts with `<` or `<?xml`).
fn is_svg(data: &[u8]) -> bool {
    let trimmed = data.iter().position(|&b| !b.is_ascii_whitespace());
    match trimmed {
        Some(pos) => data[pos] == b'<',
        None => false,
    }
}

/// Compute the printable width in pixels/dots from paper width and DPI.
///
/// Applies 4mm margins on each side (same formula used by all renderers).
fn printable_width(paper_width_mm: f64, dpi: f64) -> u32 {
    let margin_mm = 4.0;
    let printable_mm = (paper_width_mm - margin_mm * 2.0).max(paper_width_mm * 0.5);
    (printable_mm * dpi / 25.4).round().max(8.0) as u32
}

/// Convert points to pixels/dots at the given DPI.
fn pt_to_px(pt: f64, dpi: f64) -> u32 {
    (pt * dpi / 72.0).round() as u32
}

/// Extract printer configuration from AST nodes.
fn collect_printer_config(nodes: &[Node]) -> (f64, f64, u8) {
    let mut paper_width_mm = 80.0;
    let mut dpi = 203.0;
    let mut threshold = BLACK_THRESHOLD;

    for node in nodes {
        match node {
            Node::PrinterWidth { mm } => paper_width_mm = *mm,
            Node::PrinterDpi { dpi: d } => dpi = *d as f64,
            Node::PrinterThreshold { threshold: t } => threshold = *t,
            _ => {}
        }
    }

    (paper_width_mm, dpi, threshold)
}

// ─── Cache key formats ──────────────────────────────────────────────────────

/// Simple FNV-1a hash for cache keys. Returns a u64.
fn fnv_hash(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Cache key for a processed image.
fn image_cache_key(
    url: &str,
    paper_width_mm: f64,
    dpi: f64,
    max_w_pt: f64,
    max_h_pt: f64,
    threshold: u8,
    dither: bool,
) -> String {
    format!(
        "{:x}_img_{paper_width_mm}_{dpi}_{max_w_pt}_{max_h_pt}_{threshold}_{}",
        fnv_hash(url),
        if dither { "dither" } else { "threshold" }
    )
}

// ─── Resource fetching ──────────────────────────────────────────────────────

fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// SHA256 hex + extension for download cache filenames.
fn download_cache_key(url: &str) -> String {
    let hash = Sha256::digest(url.as_bytes());
    let hex = format!("{hash:x}");
    let path_part = url.split('?').next().unwrap_or(url);
    let ext = Path::new(path_part)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("bin");
    format!("{hex}.{ext}")
}

/// Load a local file, verifying it stays within resource_dir.
fn load_local(resource_dir: &Path, url: &str) -> Option<Vec<u8>> {
    let canonical_base = match fs::canonicalize(resource_dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("warning: cannot resolve base dir {}: {e}", resource_dir.display());
            return None;
        }
    };

    let joined = resource_dir.join(url);
    let canonical = match fs::canonicalize(&joined) {
        Ok(p) => p,
        Err(_) => return None,
    };

    if !canonical.starts_with(&canonical_base) {
        eprintln!("warning: path escapes base directory, skipping: {url}");
        return None;
    }

    match fs::read(&canonical) {
        Ok(bytes) => Some(bytes),
        Err(e) => {
            eprintln!("warning: cannot read {url}: {e}");
            None
        }
    }
}

/// Load bytes for a remote URL from pre-fetched resources or download cache.
///
/// The host provides bytes via `config.resources`. If a download cache is
/// configured and the URL is found there, that's used instead. Newly provided
/// bytes are written to the download cache for future runs.
fn load_remote(config: &ResourceConfig, url: &str) -> Option<Vec<u8>> {
    // Check download cache first
    if let Some(dir) = &config.cache_dir {
        let cached_path = dir.join(download_cache_key(url));
        if let Ok(bytes) = fs::read(&cached_path) {
            return Some(bytes);
        }
    }

    // Look up in host-provided resources
    let bytes = config.resources.get(url)?;

    // Write to download cache for future runs
    if let Some(dir) = &config.cache_dir {
        let _ = fs::create_dir_all(dir);
        let cached_path = dir.join(download_cache_key(url));
        if let Err(e) = fs::write(&cached_path, bytes) {
            eprintln!("warning: cannot write download cache for {url}: {e}");
        }
    }

    Some(bytes.clone())
}

/// Load raw bytes for a resource (local or remote).
fn load_raw(config: &ResourceConfig, url: &str) -> Option<Vec<u8>> {
    if is_url(url) {
        load_remote(config, url)
    } else if let Some(dir) = &config.resource_dir {
        load_local(dir, url)
    } else {
        None
    }
}

/// Discover which remote URLs need to be fetched by the host.
///
/// Walks the AST, finds all resource references (images and fonts),
/// and returns only the HTTPS URLs that are not already in the download
/// cache. The host should fetch these and put the bytes in
/// [`ResourceConfig::resources`] before rendering.
///
/// Returns an empty `Vec` if all resources are local or cached.
pub fn resolve_resources(nodes: &[Node], config: &ResourceConfig) -> Vec<String> {
    let resource_urls = rip_parser::collect_resources(nodes);
    let mut needed = Vec::new();
    let mut seen = HashSet::new();

    let all_urls = resource_urls.images.iter().chain(resource_urls.fonts.iter());

    for url in all_urls {
        if !is_url(url) || !seen.insert(url.clone()) {
            continue;
        }

        // Already provided by host?
        if config.resources.contains_key(url.as_str()) {
            continue;
        }

        // Already in download cache?
        if let Some(dir) = &config.cache_dir {
            let cached_path = dir.join(download_cache_key(url));
            if cached_path.exists() {
                continue;
            }
        }

        needed.push(url.clone());
    }

    needed
}

/// Read processed cache if it exists.
fn read_processed_cache(cache_dir: &Path, cache_key: &str) -> Option<Vec<u8>> {
    fs::read(cache_dir.join(cache_key)).ok()
}

/// Write processed data to cache.
fn write_processed_cache(cache_dir: &Path, cache_key: &str, data: &[u8]) {
    let _ = fs::create_dir_all(cache_dir);
    let path = cache_dir.join(cache_key);
    if let Err(e) = fs::write(&path, data) {
        eprintln!("warning: cannot write processed cache {cache_key}: {e}");
    }
}

// ─── Resource preparation ───────────────────────────────────────────────────

/// Prepare all resources for rendering.
///
/// Fetches fonts (passthrough TTF bytes) and images (decode → scale →
/// dither/threshold). Uses `config.cache_dir` for caching when available.
/// All renderers share the same processed images and cache keys.
pub fn prepare_resources(
    nodes: &[Node],
    config: &ResourceConfig,
) -> RenderResources {
    let (paper_width_mm, dpi, threshold) = collect_printer_config(nodes);
    let paper_width_px = printable_width(paper_width_mm, dpi);

    let mut resources = RenderResources::default();

    // Process fonts
    let mut seen_fonts = HashSet::new();
    for node in nodes {
        if let Node::Style { font, .. } = node {
            if seen_fonts.insert(font.clone()) {
                if let Some(data) = load_raw(config, font) {
                    resources.fonts.insert(font.clone(), data);
                }
            }
        }
    }

    // Process images
    let mut seen_images = HashSet::new();
    for node in nodes {
        if let Node::Image {
            url,
            width,
            height,
            dither,
            ..
        } = node
        {
            if !seen_images.insert(url.clone()) {
                continue;
            }

            let max_w_pt = width.unwrap_or(0.0);
            let max_h_pt = height.unwrap_or(0.0);

            let cache_key = image_cache_key(
                url, paper_width_mm, dpi, max_w_pt, max_h_pt, threshold, *dither,
            );

            // Check processed cache first
            if let Some(dir) = &config.cache_dir {
                if let Some(cached) = read_processed_cache(dir, &cache_key) {
                    if let Some(img) = ImageData::from_cache_bytes(&cached) {
                        resources.images.insert(url.clone(), img);
                        continue;
                    }
                }
            }

            // Load raw bytes
            let Some(raw_bytes) = load_raw(config, url) else {
                continue;
            };

            let is_svg_file = is_svg(&raw_bytes)
                || url.ends_with(".svg")
                || url.ends_with(".SVG");

            let max_w = if max_w_pt > 0.0 {
                Some(pt_to_px(max_w_pt, dpi).min(paper_width_px))
            } else {
                None
            };
            let max_h = if max_h_pt > 0.0 {
                Some(pt_to_px(max_h_pt, dpi))
            } else {
                None
            };

            let processed = if is_svg_file {
                let target_w = max_w.unwrap_or(paper_width_px);
                let target_h = max_h.unwrap_or(target_w);
                decode_svg(&raw_bytes, target_w, target_h)
            } else {
                decode_image(&raw_bytes).map(|raw| {
                    let (scaled_w, scaled_h) =
                        scale_image_dims(raw.width, raw.height, max_w, max_h, paper_width_px);
                    let pixels = scale_nn(
                        &raw.pixels,
                        raw.width,
                        raw.height,
                        scaled_w,
                        scaled_h,
                    );
                    ImageData {
                        width: scaled_w,
                        height: scaled_h,
                        pixels,
                    }
                })
            };

            if let Some(mut img) = processed {
                if *dither {
                    dither::floyd_steinberg(&mut img.pixels, img.width, img.height);
                } else {
                    for p in &mut img.pixels {
                        *p = if *p < threshold { 0 } else { 255 };
                    }
                }

                if let Some(dir) = &config.cache_dir {
                    write_processed_cache(dir, &cache_key, &img.to_cache_bytes());
                }
                resources.images.insert(url.clone(), img);
            }
        }
    }

    resources
}
