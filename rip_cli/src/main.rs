use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::{Duration, Instant};

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

enum PrintMode {
    Escpos,
    Raster,
}

enum Output {
    File(String),
    Print(String, PrintMode),
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

    let mut print_port: Option<String> = None;
    let mut use_raster = false;

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
            "--print" => {
                i += 1;
                if i >= raw.len() {
                    eprintln!("error: --print requires a port argument (e.g. COM5, /dev/usb/lp0)");
                    process::exit(1);
                }
                print_port = Some(raw[i].clone());
            }
            "--raster" => {
                use_raster = true;
            }
            _ => {
                positional.push(raw[i].clone());
            }
        }
        i += 1;
    }

    // --print only needs the input file
    if print_port.is_some() {
        if positional.is_empty() {
            usage();
        }
        let rip_path = positional[0].clone();
        let mode = if use_raster { PrintMode::Raster } else { PrintMode::Escpos };
        let base_dir = base_dir.unwrap_or_else(|| {
            Path::new(&rip_path)
                .parent()
                .unwrap_or(Path::new("."))
                .to_path_buf()
        });
        return Args {
            rip_path,
            output: Output::Print(print_port.unwrap(), mode),
            base_dir,
            cache_dir,
        };
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

fn parse_rip(args: &Args) -> Vec<rip::Node> {
    let source = fs::read_to_string(&args.rip_path).unwrap_or_else(|e| {
        eprintln!("error: cannot read {}: {e}", args.rip_path);
        process::exit(1);
    });
    rip::parse(&source)
}

fn make_config(args: &Args) -> rip::ResourceConfig {
    rip::ResourceConfig {
        resource_dir: Some(args.base_dir.clone()),
        cache_dir: args.cache_dir.clone(),
        resources: HashMap::new(),
    }
}

/// Fetch any remote resources needed for rendering.
///
/// Calls `resolve_resources` to find URLs not in cache, fetches each with
/// ureq, and populates `config.resources`. Warnings are printed for failures
/// but do not stop rendering — the image will just be missing.
fn fetch_remote_resources(nodes: &[rip::Node], config: &mut rip::ResourceConfig) {
    let needed = rip::resolve_resources(nodes, config);
    for url in &needed {
        match ureq::get(url).call() {
            Ok(response) => {
                match response.into_body().read_to_vec() {
                    Ok(bytes) => {
                        config.resources.insert(url.clone(), bytes);
                    }
                    Err(e) => {
                        eprintln!("warning: failed to read {url}: {e}");
                    }
                }
            }
            Err(e) => {
                eprintln!("warning: failed to fetch {url}: {e}");
            }
        }
    }
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

    let nodes = parse_rip(args);
    let mut config = make_config(args);
    fetch_remote_resources(&nodes, &mut config);

    // Warmup: render once so cache and OS page faults
    // are all settled before we start timing.
    let _ = rip::render_image(&nodes, &config);

    let name = Path::new(&args.rip_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&args.rip_path);

    eprintln!("benchmarking {name} ({iterations} iterations):");

    bench_fn("render_image", iterations, || {
        let _ = rip::render_image(&nodes, &config);
    });

    bench_fn("render_raster", iterations, || {
        let _ = rip::render_raster(&nodes, &config);
    });

    bench_fn("render_escpos", iterations, || {
        let _ = rip::render_escpos(&nodes, &config);
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

fn render_to_file(nodes: &[rip::Node], config: &rip::ResourceConfig, out_path: &str) {
    let ext = Path::new(out_path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    match ext {
        "png" => {
            let png_bytes = rip::render_image(nodes, config).unwrap_or_else(|e| {
                eprintln!("error: render failed: {e}");
                process::exit(1);
            });
            fs::write(out_path, &png_bytes).unwrap_or_else(|e| {
                eprintln!("error: cannot write PNG: {e}");
                process::exit(1);
            });
            eprintln!("wrote {out_path} ({} bytes)", png_bytes.len());
        }
        "raster" => {
            let raster_bytes = rip::render_raster(nodes, config).unwrap_or_else(|e| {
                eprintln!("error: render failed: {e}");
                process::exit(1);
            });
            fs::write(out_path, &raster_bytes).unwrap_or_else(|e| {
                eprintln!("error: cannot write raster: {e}");
                process::exit(1);
            });
            eprintln!("wrote {out_path} ({} bytes)", raster_bytes.len());
        }
        "bin" => {
            let bytes = rip::render_escpos(nodes, config);
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
// Print to device
// ---------------------------------------------------------------------------

fn print_to_port(nodes: &[rip::Node], config: &rip::ResourceConfig, port: &str, mode: &PrintMode) {
    let bytes = match mode {
        PrintMode::Escpos => {
            let data = rip::render_escpos(nodes, config);
            eprintln!("ESC/POS: {} bytes", data.len());
            data
        }
        PrintMode::Raster => {
            let data = rip::render_raster(nodes, config).unwrap_or_else(|e| {
                eprintln!("error: raster render failed: {e}");
                process::exit(1);
            });
            eprintln!("raster: {} bytes", data.len());
            data
        }
    };

    eprintln!("sending to {port}...");

    let mut file = fs::OpenOptions::new()
        .write(true)
        .open(port)
        .unwrap_or_else(|e| {
            eprintln!("error: cannot open {port}: {e}");
            process::exit(1);
        });

    use std::io::Write;
    file.write_all(&bytes).unwrap_or_else(|e| {
        eprintln!("error: write to {port} failed: {e}");
        process::exit(1);
    });

    eprintln!("done — sent {} bytes to {port}", bytes.len());
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn usage() -> ! {
    eprintln!("Usage: rip <file> <output> [options]");
    eprintln!("       rip <file> --print <port> [options]");
    eprintln!("       rip <file> --bench  [options]");
    eprintln!();
    eprintln!("Output formats (determined by extension):");
    eprintln!("  .png       1-bit black/white PNG (matches thermal printer)");
    eprintln!("  .raster    ESC/POS raster commands (init + GS v 0 + feed)");
    eprintln!("  .bin       ESC/POS text commands (printer's built-in fonts)");
    eprintln!("  .html      Standalone HTML document");
    eprintln!("  .txt       Plain text (monospace)");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --print <port>   Render and send to a printer device");
    eprintln!("                   (e.g. COM5, /dev/usb/lp0, /dev/ttyUSB0)");
    eprintln!("  --raster         With --print, send raster image via GS v 0");
    eprintln!("                   (default: ESC/POS text commands)");
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
            let nodes = parse_rip(&args);
            let mut config = make_config(&args);
            fetch_remote_resources(&nodes, &mut config);
            render_to_file(&nodes, &config, out_path);
        }
        Output::Print(port, mode) => {
            let nodes = parse_rip(&args);
            let mut config = make_config(&args);
            fetch_remote_resources(&nodes, &mut config);
            print_to_port(&nodes, &config, port, mode);
        }
    }
}
