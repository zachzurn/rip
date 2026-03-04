use std::alloc::{GlobalAlloc, Layout, System};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::{Duration, Instant};

use rip::{ImageData, RenderResources};
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// Tracking allocator — wraps System, counts bytes via atomics
// ---------------------------------------------------------------------------

struct TrackingAllocator;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            let current = ALLOCATED.fetch_add(layout.size(), Relaxed) + layout.size();
            PEAK.fetch_max(current, Relaxed);
            ALLOC_COUNT.fetch_add(1, Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        ALLOCATED.fetch_sub(layout.size(), Relaxed);
    }
}

fn reset_peak() {
    PEAK.store(ALLOCATED.load(Relaxed), Relaxed);
    ALLOC_COUNT.store(0, Relaxed);
}

fn peak_bytes() -> usize {
    PEAK.load(Relaxed)
}

fn alloc_count() -> usize {
    ALLOC_COUNT.load(Relaxed)
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

fn fmt_duration(d: Duration) -> String {
    let us = d.as_secs_f64() * 1_000_000.0;
    if us < 1_000.0 {
        format!("{us:.1}µs")
    } else {
        format!("{:.2}ms", us / 1_000.0)
    }
}

fn fmt_bytes(bytes: usize) -> String {
    if bytes < 1_024 {
        format!("{bytes}B")
    } else if bytes < 1_024 * 1_024 {
        format!("{:.1}KB", bytes as f64 / 1_024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1_024.0 * 1_024.0))
    }
}

fn fmt_count(n: usize) -> String {
    if n < 1_000 {
        format!("{n}")
    } else {
        format!("{},{:03}", n / 1_000, n % 1_000)
    }
}

// ---------------------------------------------------------------------------
// CLI arguments
// ---------------------------------------------------------------------------

enum Output {
    File(String),
    Bench,
}

struct Args {
    rip_path: String,
    output: Output,
    base_dir: PathBuf,
    cache_dir: Option<PathBuf>,
}

fn parse_args() -> Args {
    let raw: Vec<String> = std::env::args().skip(1).collect();

    let mut base_dir: Option<PathBuf> = None;
    let mut cache_dir: Option<PathBuf> = None;
    let mut positional: Vec<String> = Vec::new();

    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--base" => {
                i += 1;
                if i >= raw.len() {
                    eprintln!("error: --base requires a folder argument");
                    process::exit(1);
                }
                base_dir = Some(PathBuf::from(&raw[i]));
            }
            "--cache" => {
                i += 1;
                if i >= raw.len() {
                    eprintln!("error: --cache requires a folder argument");
                    process::exit(1);
                }
                cache_dir = Some(PathBuf::from(&raw[i]));
            }
            _ => {
                positional.push(raw[i].clone());
            }
        }
        i += 1;
    }

    if positional.len() < 2 {
        usage();
    }

    let rip_path = positional[0].clone();
    let output = if positional[1] == "--bench" {
        Output::Bench
    } else {
        Output::File(positional[1].clone())
    };

    // Default base_dir to the .rip file's parent directory
    let base_dir = base_dir.unwrap_or_else(|| {
        Path::new(&rip_path)
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf()
    });

    Args { rip_path, output, base_dir, cache_dir }
}

// ---------------------------------------------------------------------------
// Resource loading
// ---------------------------------------------------------------------------

const MAX_DOWNLOAD_BYTES: u64 = 10 * 1024 * 1024; // 10MB

fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// Load a local file, verifying it stays within base_dir.
fn load_local(url: &str, base_dir: &Path) -> Option<Vec<u8>> {
    let canonical_base = match fs::canonicalize(base_dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("warning: cannot resolve base dir {}: {e}", base_dir.display());
            return None;
        }
    };

    let joined = base_dir.join(url);
    let canonical = match fs::canonicalize(&joined) {
        Ok(p) => p,
        Err(_) => return None, // file doesn't exist, silently skip
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

/// Build a cache filename from a URL: sha256 hex + original extension.
fn cache_key(url: &str) -> String {
    let hash = Sha256::digest(url.as_bytes());
    let hex = format!("{hash:x}");

    // Preserve extension from URL (strip query string first)
    let path_part = url.split('?').next().unwrap_or(url);
    let ext = Path::new(path_part)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("bin");

    format!("{hex}.{ext}")
}

/// Download a remote URL, using cache_dir if provided.
fn load_remote(url: &str, cache_dir: Option<&Path>) -> Option<Vec<u8>> {
    // Check cache first
    if let Some(dir) = cache_dir {
        let cached_path = dir.join(cache_key(url));
        if let Ok(bytes) = fs::read(&cached_path) {
            return Some(bytes);
        }
    }

    // Download
    eprintln!("downloading {url}...");
    let response = match ureq::get(url).call() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("warning: download failed for {url}: {e}");
            return None;
        }
    };

    let mut bytes = Vec::new();
    match response
        .into_body()
        .with_config()
        .limit(MAX_DOWNLOAD_BYTES)
        .reader()
        .read_to_end(&mut bytes)
    {
        Ok(_) => {}
        Err(e) => {
            eprintln!("warning: failed reading response for {url}: {e}");
            return None;
        }
    }

    // Write to cache
    if let Some(dir) = cache_dir {
        let _ = fs::create_dir_all(dir);
        let cached_path = dir.join(cache_key(url));
        if let Err(e) = fs::write(&cached_path, &bytes) {
            eprintln!("warning: cannot write cache for {url}: {e}");
        }
    }

    Some(bytes)
}

/// Load raw bytes for a resource URL (local or remote).
fn load_resource(url: &str, base_dir: &Path, cache_dir: Option<&Path>) -> Option<Vec<u8>> {
    if is_url(url) {
        load_remote(url, cache_dir)
    } else {
        load_local(url, base_dir)
    }
}

/// Decode image bytes to grayscale ImageData.
fn decode_image(url: &str, bytes: &[u8]) -> Option<ImageData> {
    match image::load_from_memory(bytes) {
        Ok(img) => {
            let gray = img.to_luma8();
            Some(ImageData {
                width: gray.width(),
                height: gray.height(),
                pixels: gray.into_raw(),
            })
        }
        Err(e) => {
            eprintln!("warning: cannot decode image {url}: {e}");
            None
        }
    }
}

fn load_rip(args: &Args) -> (Vec<rip::Node>, RenderResources) {
    let source = fs::read_to_string(&args.rip_path).unwrap_or_else(|e| {
        eprintln!("error: cannot read {}: {e}", args.rip_path);
        process::exit(1);
    });

    let nodes = rip::parse(&source);
    let res = rip::collect_resources(&nodes);

    let mut resources = RenderResources::default();
    let cache = args.cache_dir.as_deref();

    for url in &res.fonts {
        if let Some(bytes) = load_resource(url, &args.base_dir, cache) {
            resources.fonts.insert(url.clone(), bytes);
        }
    }

    for url in &res.images {
        if let Some(bytes) = load_resource(url, &args.base_dir, cache) {
            if let Some(img) = decode_image(url, &bytes) {
                resources.images.insert(url.clone(), img);
            }
        }
    }

    (nodes, resources)
}

// ---------------------------------------------------------------------------
// Bench
// ---------------------------------------------------------------------------

fn bench_fn<F: FnMut()>(name: &str, iterations: u32, mut f: F) {
    // Warmup
    for _ in 0..3 {
        f();
    }

    let mut times = Vec::with_capacity(iterations as usize);
    let mut peak_max: usize = 0;
    let mut alloc_total: usize = 0;

    for _ in 0..iterations {
        reset_peak();
        let start = Instant::now();
        f();
        times.push(start.elapsed());
        peak_max = peak_max.max(peak_bytes());
        alloc_total += alloc_count();
    }

    times.sort();
    let min = times[0];
    let median = times[times.len() / 2];
    let max = times[times.len() - 1];
    let avg_allocs = alloc_total / iterations as usize;

    eprintln!(
        "  {name:<24} min={:<10} median={:<10} max={:<10} peak={:<10} allocs={}",
        fmt_duration(min),
        fmt_duration(median),
        fmt_duration(max),
        fmt_bytes(peak_max),
        fmt_count(avg_allocs),
    );
}

fn run_bench(args: &Args) {
    let iterations = 100;

    // Warmup: load + render once so downloads, cache, and OS page faults
    // are all settled before we start timing.
    let _ = load_rip(args);

    let (nodes, resources) = load_rip(args);

    let name = Path::new(&args.rip_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&args.rip_path);

    eprintln!("benchmarking {name} ({iterations} iterations):");

    bench_fn("render_luma8", iterations, || {
        let _ = rip::render_luma8(&nodes, &resources);
    });

    bench_fn("render_luma1", iterations, || {
        let _ = rip::render_luma1(&nodes, &resources);
    });

    let output = rip::render_luma8(&nodes, &resources).unwrap();
    bench_fn("pack_luma1", iterations, || {
        let _ = rip::pack_luma1(output.width, &output.pixels, rip::BLACK_THRESHOLD);
    });

    bench_fn("render_escpos", iterations, || {
        let _ = rip::render_escpos(&nodes, &resources);
    });

    bench_fn("render_html", iterations, || {
        let _ = rip::render_html(&nodes);
    });

    bench_fn("render_text", iterations, || {
        let _ = rip::render_text(&nodes);
    });
}

// ---------------------------------------------------------------------------
// Render to file
// ---------------------------------------------------------------------------

fn render_to_file(nodes: &[rip::Node], resources: &RenderResources, out_path: &str) {
    let ext = Path::new(out_path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    match ext {
        "png" => {
            let output = rip::render_luma8(nodes, resources).unwrap();
            image::save_buffer(
                out_path,
                &output.pixels,
                output.width,
                output.height,
                image::ColorType::L8,
            )
            .unwrap_or_else(|e| {
                eprintln!("error: cannot write PNG: {e}");
                process::exit(1);
            });
            eprintln!(
                "wrote {out_path} ({}x{}, {} bytes)",
                output.width,
                output.height,
                fs::metadata(out_path).map(|m| m.len()).unwrap_or(0),
            );
        }
        "raster" => {
            let output = rip::render_luma1(nodes, resources).unwrap();
            fs::write(out_path, &output.pixels).unwrap_or_else(|e| {
                eprintln!("error: cannot write raster: {e}");
                process::exit(1);
            });
            eprintln!(
                "wrote {out_path} ({}x{}, {} bytes)",
                output.width,
                output.height,
                output.pixels.len(),
            );
        }
        "bin" => {
            let bytes = rip::render_escpos(nodes, resources);
            fs::write(out_path, &bytes).unwrap_or_else(|e| {
                eprintln!("error: cannot write ESC/POS: {e}");
                process::exit(1);
            });
            eprintln!("wrote {out_path} ({} bytes)", bytes.len());
        }
        "html" => {
            let html = rip::render_html(nodes);
            fs::write(out_path, &html).unwrap_or_else(|e| {
                eprintln!("error: cannot write HTML: {e}");
                process::exit(1);
            });
            eprintln!("wrote {out_path} ({} bytes)", html.len());
        }
        "txt" => {
            let text = rip::render_text(nodes);
            fs::write(out_path, &text).unwrap_or_else(|e| {
                eprintln!("error: cannot write text: {e}");
                process::exit(1);
            });
            eprintln!("wrote {out_path} ({} bytes)", text.len());
        }
        _ => {
            eprintln!("error: unknown output extension '.{ext}'");
            usage();
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn usage() -> ! {
    eprintln!("Usage: rip <file> <output> [options]");
    eprintln!("       rip <file> --bench  [options]");
    eprintln!();
    eprintln!("Output formats (determined by extension):");
    eprintln!("  .png       8-bit grayscale PNG");
    eprintln!("  .raster    1-bit packed raster data");
    eprintln!("  .bin       ESC/POS binary commands");
    eprintln!("  .html      Standalone HTML document");
    eprintln!("  .txt       Plain text (monospace)");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --bench          Benchmark all render formats (100 iterations)");
    eprintln!("  --base <folder>  Base directory for resolving relative paths");
    eprintln!("                   (defaults to the .rip file's parent directory)");
    eprintln!("  --cache <folder> Cache downloaded images/fonts to this folder");
    process::exit(1);
}

fn main() {
    let args = parse_args();

    match &args.output {
        Output::Bench => run_bench(&args),
        Output::File(out_path) => {
            let (nodes, resources) = load_rip(&args);
            render_to_file(&nodes, &resources, out_path);
        }
    }
}
