/**
 * rip-js — Rip receipt markup rendered via WASM.
 *
 * All methods are static and async. WASM is lazy-initialized on first call.
 * Resources (images, fonts) referenced in markup are fetched automatically.
 *
 * @example
 * import { Rip } from 'rip-js';
 *
 * const html   = await Rip.renderHtml(markup);
 * const text   = await Rip.renderText(markup);
 * const pixels = await Rip.renderPixels(markup);
 * const escpos = await Rip.renderEscpos(markup);
 */

import wasmInit, * as wasm from './rip_wasm.js';

const isNode = typeof globalThis.process?.versions?.node === 'string';

// ─── Lazy WASM init ─────────────────────────────────────────────────

let initDone = false;
let initPromise = null;

function ensureInit() {
  if (initDone) return Promise.resolve();
  if (!initPromise) {
    initPromise = (async () => {
      if (isNode) {
        // Node.js: read the .wasm file from disk and pass bytes to init
        const { readFile } = await import('node:fs/promises');
        const { fileURLToPath } = await import('node:url');
        const { dirname, join } = await import('node:path');
        const dir = dirname(fileURLToPath(import.meta.url));
        const bytes = await readFile(join(dir, 'rip_wasm_bg.wasm'));
        await wasmInit({ module_or_path: bytes });
      } else {
        // Browser: default init fetches .wasm relative to import.meta.url
        await wasmInit();
      }
      initDone = true;
    })();
  }
  return initPromise;
}

// ─── Resource loading ───────────────────────────────────────────────

/**
 * Load raw bytes from a URL or local file path.
 *
 * - HTTP/HTTPS URLs → fetch() (works in browser + Node 18+)
 * - Browser relative paths → fetch() (resolves against page origin)
 * - Node.js local paths → fs.readFile()
 */
async function loadBytes(urlOrPath) {
  // Node.js + local path → read from filesystem
  if (isNode && !/^https?:\/\//i.test(urlOrPath)) {
    const { readFile } = await import('node:fs/promises');
    const buf = await readFile(urlOrPath);
    return new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength);
  }
  // Everything else → fetch (handles relative URLs in browser)
  const res = await fetch(urlOrPath);
  if (!res.ok) {
    throw new Error(`Failed to fetch ${urlOrPath}: ${res.status} ${res.statusText}`);
  }
  return new Uint8Array(await res.arrayBuffer());
}

/**
 * Decode raw image bytes to luma8 grayscale.
 *
 * - Browser: createImageBitmap + OffscreenCanvas (zero deps)
 * - Node.js: sharp (must be installed: npm install sharp)
 */
async function decodeImage(bytes) {
  if (!isNode) {
    // Browser: native image decoding
    const blob = new Blob([bytes]);
    const bitmap = await createImageBitmap(blob);
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    const ctx = canvas.getContext('2d');
    ctx.drawImage(bitmap, 0, 0);
    const { data } = ctx.getImageData(0, 0, bitmap.width, bitmap.height);
    // RGBA → luma8 (ITU-R BT.601 luminance)
    const pixels = new Uint8Array(bitmap.width * bitmap.height);
    for (let i = 0; i < pixels.length; i++) {
      const j = i * 4;
      pixels[i] = Math.round(0.299 * data[j] + 0.587 * data[j + 1] + 0.114 * data[j + 2]);
    }
    bitmap.close();
    return { width: bitmap.width, height: bitmap.height, pixels };
  }

  // Node.js: use sharp for image decoding
  try {
    const sharp = (await import('sharp')).default;
    const { data, info } = await sharp(Buffer.from(bytes))
      .greyscale()
      .raw()
      .toBuffer({ resolveWithObject: true });
    return {
      width: info.width,
      height: info.height,
      pixels: new Uint8Array(data.buffer, data.byteOffset, data.byteLength),
    };
  } catch (e) {
    if (e.code === 'ERR_MODULE_NOT_FOUND' || e.code === 'MODULE_NOT_FOUND') {
      throw new Error(
        'Image decoding in Node.js requires the "sharp" package. Install it with: npm install sharp'
      );
    }
    throw e;
  }
}

/**
 * Parse markup, discover resource URLs, fetch and decode them.
 *
 * Returns a resources object ready for the Rust render functions:
 * { images: { url: { width, height, pixels } }, fonts: { url: Uint8Array } }
 */
async function loadResources(source) {
  const { fonts, images } = wasm.get_resources(source);
  const resources = { images: {}, fonts: {} };

  await Promise.all([
    ...images.map(async (url) => {
      const bytes = await loadBytes(url);
      resources.images[url] = await decodeImage(bytes);
    }),
    ...fonts.map(async (url) => {
      resources.fonts[url] = await loadBytes(url);
    }),
  ]);

  return resources;
}

/**
 * Check if the markup references any external resources.
 */
function needsResources(source) {
  const { fonts, images } = wasm.get_resources(source);
  return fonts.length > 0 || images.length > 0;
}

// ─── Public API ─────────────────────────────────────────────────────

export class Rip {
  /**
   * Render markup to a standalone HTML document.
   * @param {string} source - Rip markup
   * @returns {Promise<string>} HTML string
   */
  static async renderHtml(source) {
    await ensureInit();
    return wasm.render_html(source);
  }

  /**
   * Render markup to plain text (monospace ASCII).
   * @param {string} source - Rip markup
   * @returns {Promise<string>} Plain text string
   */
  static async renderText(source) {
    await ensureInit();
    return wasm.render_text(source);
  }

  /**
   * Render markup to 8-bit grayscale pixels (anti-aliased).
   * @param {string} source - Rip markup
   * @returns {Promise<{width: number, height: number, pixels: Uint8Array, dirtyRows: boolean[]}>}
   */
  static async renderPixels(source) {
    await ensureInit();
    const resources = needsResources(source) ? await loadResources(source) : {};
    return wasm.render_pixels(source, resources);
  }

  /**
   * Render markup to 1-bit packed pixels (thresholded black/white).
   * @param {string} source - Rip markup
   * @returns {Promise<{width: number, height: number, pixels: Uint8Array, dirtyRows: boolean[]}>}
   */
  static async renderRaster(source) {
    await ensureInit();
    const resources = needsResources(source) ? await loadResources(source) : {};
    return wasm.render_raster(source, resources);
  }

  /**
   * Render markup to ESC/POS binary commands for thermal printers.
   * @param {string} source - Rip markup
   * @returns {Promise<Uint8Array>}
   */
  static async renderEscpos(source) {
    await ensureInit();
    const resources = needsResources(source) ? await loadResources(source) : {};
    return wasm.render_escpos(source, resources);
  }
}
