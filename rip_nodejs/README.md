# Rip (npm `rip-receipt`)

Native Node.js addon for the [Rip](https://github.com/zachzurn/rip) receipt markup language. Render receipts to PNG, HTML, plain text, or ESC/POS binary — all from a simple markup syntax.

Built with [napi-rs](https://napi.rs) for maximum performance. All rendering runs on libuv's thread pool and never blocks the event loop. No WASM, no `sharp`, no JS-side image decoding.

## Install

```bash
npm install rip-receipt
```

## Quick Start

```javascript
import { parse, renderHtml, renderImage, renderEscpos } from 'rip-receipt';

// Simple receipt
const markup = `
#### BURGER BARN ####
|> 742 Evergreen Terrace <|
===
| Classic Burger  |>  $8.99 |
| Cheese Fries    |>  $4.50 |
---
++ | *TOTAL*      |> *$13.49* | ++
@cut()
`;

// Parse once, render many ways
const doc = parse(markup);

// HTML — standalone document with inline styles and SVG barcodes/QR
const html = await renderHtml(doc);

// PNG image — 1-bit black/white, matches thermal printer output
const png = await renderImage(doc, { resourceDir: './assets' });
fs.writeFileSync('receipt.png', png);

// ESC/POS — send directly to a thermal printer
const escpos = await renderEscpos(doc, { resourceDir: './assets' });
```

## API

### `parse(source) → Document`

Parses receipt markup into a `Document`. The document can be passed to `resolveResources()` and all `render*()` functions. Synchronous and fast.

### `resolveResources(doc, config?) → string[]`

Returns an array of HTTPS URLs that need to be fetched before rendering. URLs already in the download cache are excluded. Returns an empty array if all resources are local or cached.

```javascript
const doc = parse(markup);
const needed = resolveResources(doc, { cacheDir: './cache' });

// Fetch with your preferred HTTP client
const resources = {};
for (const url of needed) {
    const res = await fetch(url);
    resources[url] = Buffer.from(await res.arrayBuffer());
}

// Render with pre-fetched resources
const png = await renderImage(doc, { resourceDir: './assets', cacheDir: './cache', resources });
```

### `renderImage(doc, config?) → Promise<Buffer>`

Renders a Document to a 1-bit black/white PNG image. Matches thermal printer appearance.

### `renderRaster(doc, config?) → Promise<Buffer>`

Renders a Document to ESC/POS raster print commands. Returns the complete byte stream (init + raster image + feed). Send directly to a thermal printer.

### `renderEscpos(doc, config?) → Promise<Buffer>`

Renders a Document to ESC/POS binary using the printer's built-in text engine. Images are embedded as inline raster data.

### `renderHtml(doc) → Promise<string>`

Renders a Document to a standalone HTML document with inline styles. QR codes and barcodes are inline SVG. No resource config needed.

### `renderText(doc) → Promise<string>`

Renders a Document to plain monospace text.

### Convenience functions

Each render function has a `*FromMarkup` variant that takes a markup string directly. Useful when you don't need `resolveResources()`:

```javascript
import { renderHtmlFromMarkup } from 'rip-receipt';

const html = await renderHtmlFromMarkup('## Hello\n---\n| Item |> $5.00 |');
```

Available: `renderImageFromMarkup`, `renderRasterFromMarkup`, `renderEscposFromMarkup`, `renderHtmlFromMarkup`, `renderTextFromMarkup`.

### `RenderConfig`

Optional config for `resolveResources()`, `renderImage`, `renderRaster`, and `renderEscpos`:

```typescript
interface RenderConfig {
  /** Base directory for resolving relative resource paths (images, etc.) */
  resourceDir?: string;
  /** Directory for caching downloaded and processed resources */
  cacheDir?: string;
  /** Pre-fetched remote resource bytes, keyed by URL */
  resources?: Record<string, Buffer>;
}
```

## Resources & Caching

Images and other resources referenced in markup (via `@image()`, `@logo()`, etc.):

- **Local files** — resolved relative to `resourceDir`, loaded by Rust
- **Remote URLs** — use `resolveResources()` to discover what needs fetching, then pass bytes via `config.resources`
- **Caching** — set `cacheDir` to enable disk caching of downloaded and processed resources

```javascript
import { parse, resolveResources, renderImage } from 'rip-receipt';

const doc = parse('@image(logo.png)\n## Receipt\n| Coffee |> $4.50 |');
const png = await renderImage(doc, {
  resourceDir: './assets',
  cacheDir: './cache',
});
```

## Markup Syntax

See the full [language spec](https://github.com/zachzurn/rip/blob/main/SPEC.md). The short version:

- **Text**: just type it
- **Styles**: `*bold*` `_underline_` `` `italic` `` `~strikethrough~`
- **Sizes**: `## header ##` (more `#` = bigger), `++ body ++` (more `+` = bigger)
- **Columns**: `| left | right |` with `>` / `<` for alignment
- **Dividers**: `---` thin, `===` thick, `...` dotted
- **Directives**: `@image()` `@qr()` `@barcode()` `@cut()` `@feed()` `@drawer()` `@logo()` `@printer-width()` `@printer-dpi()`

## Prerequisites (building from source)

```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build native addon
cd rip_nodejs
npm install
npm run build
```

## Project Structure

```
rip_nodejs/
  src/lib.rs         # Rust napi-rs bindings (AsyncTask pattern)
  index.js           # CJS loader for the native .node addon
  index.d.ts         # TypeScript declarations (auto-generated)
  package.json       # npm package metadata
  test_smoke.mjs     # Smoke tests (all renderers)
  test_resources.mjs # Resource + caching tests
```

## Test

```bash
node test_smoke.mjs      # 32 tests — all renderers, Document + FromMarkup
node test_resources.mjs  # 12 tests — local images, caching, ESC/POS
```

## License

MIT OR Apache-2.0
