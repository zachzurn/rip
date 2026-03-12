# Rip (npm `rip-receipt`)

Native Node.js addon for the [Rip](https://github.com/zachzurn/rip) receipt markup language. Render receipts to PNG, HTML, plain text, or ESC/POS binary — all from a simple markup syntax.

Built with [napi-rs](https://napi.rs) for maximum performance. All rendering runs on libuv's thread pool and never blocks the event loop. No WASM, no `sharp`, no JS-side image decoding.

## Install

```bash
npm install rip-receipt
```

## Quick Start

```javascript
import { renderHtml, renderImage, renderEscpos } from 'rip-receipt';

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

// HTML — standalone document with inline styles and SVG barcodes/QR
const html = await renderHtml(markup);

// PNG image — 1-bit black/white, matches thermal printer output
const png = await renderImage(markup);
fs.writeFileSync('receipt.png', png);

// ESC/POS — send directly to a thermal printer
const escpos = await renderEscpos(markup);
```

## API

All functions are async and return a `Promise`.

### `renderImage(source, config?) → Promise<Buffer>`

Renders markup to a 1-bit black/white PNG image. Matches thermal printer appearance.

### `renderRaster(source, config?) → Promise<Buffer>`

Renders markup to ESC/POS raster print commands. Returns the complete byte stream (init + raster image + feed). Send directly to a thermal printer.

### `renderEscpos(source, config?) → Promise<Buffer>`

Renders markup to ESC/POS binary using the printer's built-in text engine. Images are embedded as inline raster data.

### `renderHtml(source) → Promise<string>`

Renders markup to a standalone HTML document with inline styles. QR codes and barcodes are inline SVG. No resource config needed.

### `renderText(source) → Promise<string>`

Renders markup to plain monospace text.

### `RenderConfig`

Optional second argument for `renderImage`, `renderRaster`, and `renderEscpos`:

```typescript
interface RenderConfig {
  /** Base directory for resolving relative resource paths (images, etc.) */
  resourceDir?: string;
  /** Directory for caching downloaded and processed resources */
  cacheDir?: string;
}
```

## Resources & Caching

Images and other resources referenced in markup (via `@image()`, `@logo()`, etc.) are handled automatically by the Rust runtime:

- **Local files** — resolved relative to `resourceDir`
- **HTTPS URLs** — fetched automatically (works even without `resourceDir`)
- **Caching** — set `cacheDir` to enable disk caching of downloaded and processed resources

```javascript
import { renderImage } from 'rip-receipt';

const png = await renderImage('@image(logo.png)\n## Receipt\n| Coffee |> $4.50 |', {
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
  index.js           # ESM loader for the native .node addon
  index.d.ts         # TypeScript declarations (auto-generated)
  package.json       # npm package metadata
  test_smoke.mjs     # Smoke tests (all renderers)
  test_resources.mjs # Resource + caching tests
```

## Test

```bash
node test_smoke.mjs      # 19 tests — all renderers
node test_resources.mjs  # 8 tests — local images, caching, ESC/POS
```

## License

MIT OR Apache-2.0
