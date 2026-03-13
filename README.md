# Rip

A markup language for receipts. Parse it, render it to pixels, HTML, plain text, or ESC/POS binary.

Jump to: [Syntax](#syntax) · [JavaScript Quick Start](#javascript-quick-start) · [CLI Quick Start](#cli-quick-start) · [Rust Quick Start](#rust-quick-start)

```
#### BURGER BARN ####
|> 742 Evergreen Terrace <|

===

| Classic Burger  |>   $8.99 |
| Cheese Fries    |>   $4.50 |

---

++ | *TOTAL*      |> *$13.49* | ++

@cut()
```

![Rendered receipt example](examples/readme-example.png)

## Syntax

See [SPEC.md](SPEC.md) for the full language reference. The short version:

- **Text**: just type it
- **Styles**: `*bold*` `_underline_` `` `italic` `` `~strikethrough~`
- **Sizes**: `## header ##` (more `#` = bigger), `++ body ++` (more `+` = bigger)
- **Columns**: `| left | right |` with `>` / `<` for alignment
- **Dividers**: `---` thin, `===` thick, `...` dotted
- **Directives**: `@image()` `@qr()` `@barcode()` `@cut()` `@feed()` `@drawer()` `@style()` `@printer-width()` `@printer-dpi()` `@printer-threshold()`

## JavaScript Quick Start

```bash
npm install rip-receipt
```

```javascript
import { parse, renderHtml, renderText, renderImage, renderRaster, renderEscpos } from 'rip-receipt';

// Parse once, render many ways
const doc = parse("## Hello\n---\n| Item |> $5.00 |");

// HTML — standalone document with inline styles and SVG barcodes/QR
const html = await renderHtml(doc);

// Plain text
const text = await renderText(doc);

// 1-bit PNG image (matches thermal printer output)
const png = await renderImage(doc, { resourceDir: './assets' });

// ESC/POS raster print commands
const raster = await renderRaster(doc, { resourceDir: './assets' });

// ESC/POS binary commands (text engine + inline images)
const escpos = await renderEscpos(doc, { resourceDir: './assets' });
```

Local images are loaded by the native Rust runtime — no `sharp` or JS-side image handling needed. For remote URLs, use `resolveResources()` to discover what needs fetching and pass bytes via `config.resources`.

## CLI Quick Start

```
cargo install --path rip_cli

rip receipt.rip output.png      # grayscale PNG
rip receipt.rip output.html     # HTML
rip receipt.rip output.txt      # plain text
rip receipt.rip output.bin      # ESC/POS binary
rip receipt.rip output.raster   # 1-bit packed raster
rip receipt.rip --bench         # benchmark all renderers
```

## Rust Quick Start

```rust
let nodes = rip::parse("## Hello\n---\n| Item |> $5.00 |");
let config = rip::ResourceConfig::default();

// 1-bit PNG image
let png = rip::render_image(&nodes, &config).unwrap();

// HTML
let html = rip::render_html(&nodes);

// Plain text
let text = rip::render_text(&nodes);

// ESC/POS binary (thermal printer commands)
let escpos = rip::render_escpos(&nodes, &config);
```

## Crates

| Crate | What it does |
|---|---|
| `rip` | Unified API — start here |
| `rip_parser` | Parses `.rip` markup into an AST |
| `rip_image` | Renders to grayscale pixels |
| `rip_html` | Renders to standalone HTML |
| `rip_text` | Renders to plain text |
| `rip_escpos` | Renders to ESC/POS binary |
| `rip_resources` | Local file loading, image decoding, caching |
| `rip_cli` | CLI tool for rendering files |
| `rip_nodejs` | Native Node.js addon → npm [`rip-receipt`](https://www.npmjs.com/package/rip-receipt) |
| `rip_android` | Android/Kotlin bindings via JNI |

## How resources work

Images, QR codes, and barcodes referenced in markup are handled by `rip_resources`:

- **Local files** — resolved relative to a `resource_dir` you provide
- **Remote URLs** — the host fetches them; use `resolve_resources()` to discover what's needed, then pass bytes via `config.resources`
- **Caching** — optional `cache_dir` enables disk caching of downloads and processed images

```rust
use rip::{parse, resolve_resources, render_image, ResourceConfig};

let nodes = parse("@image(logo.png)\n## Receipt");
let mut config = ResourceConfig {
    resource_dir: Some("./assets".into()),
    cache_dir: Some("./cache".into()),
    ..Default::default()
};

// For remote URLs: fetch what's needed, then populate config.resources
let needed = resolve_resources(&nodes, &config);
for url in &needed {
    // fetch url bytes with your preferred HTTP client...
    // config.resources.insert(url.clone(), bytes);
}

let png = render_image(&nodes, &config).unwrap();
```

Each host brings its own HTTP client — Node.js uses `fetch()`, Android uses OkHttp, the CLI uses `ureq`. The core library does zero network I/O, keeping the binary small and cross-compilation simple.

## License

MIT OR Apache-2.0

## LLM Use

Claude Code was heavily used in the creation of the code in this project.
