use std::alloc::{GlobalAlloc, Layout, System};
use std::fs;
use std::path::Path;
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::{Duration, Instant};

use rip::{ImageData, RenderResources};

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
// Resource loading
// ---------------------------------------------------------------------------

fn load_rip(rip_path: &str) -> (Vec<rip::Node>, RenderResources) {
    let source = fs::read_to_string(rip_path).unwrap_or_else(|e| {
        eprintln!("error: cannot read {rip_path}: {e}");
        process::exit(1);
    });

    let base_dir = Path::new(rip_path).parent().unwrap();
    let nodes = rip::parse(&source);
    let res = rip::collect_resources(&nodes);

    let mut resources = RenderResources::default();

    for url in &res.fonts {
        let path = base_dir.join(url);
        if let Ok(bytes) = fs::read(&path) {
            resources.fonts.insert(url.clone(), bytes);
        }
    }

    for url in &res.images {
        let path = base_dir.join(url);
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(img) = image::load_from_memory(&bytes) {
                let gray = img.to_luma8();
                resources.images.insert(
                    url.clone(),
                    ImageData {
                        width: gray.width(),
                        height: gray.height(),
                        pixels: gray.into_raw(),
                    },
                );
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

fn run_bench(rip_path: &str) {
    let iterations = 100;
    let (nodes, resources) = load_rip(rip_path);

    let name = Path::new(rip_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(rip_path);

    eprintln!("benchmarking {name} ({iterations} iterations):");

    bench_fn("render_luma8", iterations, || {
        let _ = rip::render_luma8(&nodes, &resources);
    });

    bench_fn("render_luma1", iterations, || {
        let _ = rip::render_luma1(&nodes, &resources);
    });

    let output = rip::render_luma8(&nodes, &resources).unwrap();
    bench_fn("pack_luma1", iterations, || {
        let _ = rip::pack_luma1(output.width, &output.pixels);
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
    eprintln!("Usage: rip <file> <output>");
    eprintln!("       rip <file> --bench");
    eprintln!();
    eprintln!("Output formats (determined by extension):");
    eprintln!("  .png       8-bit grayscale PNG");
    eprintln!("  .raster    1-bit packed raster data");
    eprintln!("  .bin       ESC/POS binary commands");
    eprintln!("  .html      Standalone HTML document");
    eprintln!("  .txt       Plain text (monospace)");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --bench    Benchmark all render formats (100 iterations)");
    process::exit(1);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        usage();
    }

    let rip_path = &args[1];

    if args[2] == "--bench" {
        run_bench(rip_path);
    } else {
        let out_path = &args[2];
        let (nodes, resources) = load_rip(rip_path);
        render_to_file(&nodes, &resources, out_path);
    }
}
