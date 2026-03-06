/**
 * rip-receipt — Rip receipt markup rendered via WASM.
 *
 * All methods are static and async. WASM is lazy-initialized on first call.
 * Resources (images, fonts) referenced in markup are fetched automatically.
 *
 * @example
 * import { Rip } from 'rip-receipt';
 *
 * Rip.configure({ basePath: '/assets/', cachePath: './.rip-cache' });
 *
 * const html   = await Rip.renderHtml(markup);
 * const pixels = await Rip.renderPixels(markup);
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
        const { readFile } = await import('node:fs/promises');
        const { fileURLToPath } = await import('node:url');
        const { dirname, join } = await import('node:path');
        const dir = dirname(fileURLToPath(import.meta.url));
        const bytes = await readFile(join(dir, 'rip_wasm_bg.wasm'));
        await wasmInit({ module_or_path: bytes });
      } else {
        await wasmInit();
      }
      initDone = true;
    })();
  }
  return initPromise;
}

// ─── Configuration ──────────────────────────────────────────────────

let config = {
  basePath: '',
  cachePath: '',
};

// In-memory cache: resolved URL → decoded resource
const memCache = new Map();

// ─── Resource loading ───────────────────────────────────────────────

/**
 * Resolve a resource path against the configured basePath.
 */
function resolveUrl(urlOrPath) {
  // Already absolute URL — don't modify
  if (/^https?:\/\//i.test(urlOrPath)) return urlOrPath;
  // Has a basePath — prepend it
  if (config.basePath) {
    const base = config.basePath.endsWith('/') ? config.basePath : config.basePath + '/';
    return base + urlOrPath;
  }
  return urlOrPath;
}

/**
 * Get the disk cache file path for a URL (Node.js only).
 * Returns null if cachePath is not configured.
 */
function diskCacheKey(url) {
  if (!config.cachePath || !isNode) return null;
  // Simple hash: replace non-alphanumeric chars with underscores
  const safe = url.replace(/[^a-zA-Z0-9._-]/g, '_');
  return null; // placeholder, will be set after imports
}

let _pathJoin = null;
let _fsp = null;

async function ensureNodeImports() {
  if (_fsp) return;
  _fsp = await import('node:fs/promises');
  const path = await import('node:path');
  _pathJoin = path.join;
}

async function diskCachePath(url) {
  if (!config.cachePath || !isNode) return null;
  await ensureNodeImports();
  const safe = url.replace(/[^a-zA-Z0-9._-]/g, '_');
  return _pathJoin(config.cachePath, safe);
}

/**
 * Try reading from disk cache. Returns Uint8Array or null.
 */
async function readDiskCache(url) {
  const path = await diskCachePath(url);
  if (!path) return null;
  try {
    const buf = await _fsp.readFile(path);
    return new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength);
  } catch {
    return null;
  }
}

/**
 * Write bytes to disk cache.
 */
async function writeDiskCache(url, bytes) {
  const path = await diskCachePath(url);
  if (!path) return;
  try {
    await _fsp.mkdir(config.cachePath, { recursive: true });
    await _fsp.writeFile(path, bytes);
  } catch {
    // Silently ignore cache write failures
  }
}

/**
 * Load raw bytes from a URL or local file path.
 */
async function loadBytes(resolvedUrl) {
  // Node.js + local path → read from filesystem
  if (isNode && !/^https?:\/\//i.test(resolvedUrl)) {
    await ensureNodeImports();
    const buf = await _fsp.readFile(resolvedUrl);
    return new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength);
  }
  // Everything else → fetch
  const res = await fetch(resolvedUrl);
  if (!res.ok) {
    throw new Error(`Failed to fetch ${resolvedUrl}: ${res.status} ${res.statusText}`);
  }
  return new Uint8Array(await res.arrayBuffer());
}

/**
 * Load bytes with disk cache support.
 */
async function loadBytesWithCache(resolvedUrl) {
  // Try disk cache first
  const cached = await readDiskCache(resolvedUrl);
  if (cached) return cached;
  // Fetch
  const bytes = await loadBytes(resolvedUrl);
  // Write to disk cache
  await writeDiskCache(resolvedUrl, bytes);
  return bytes;
}

/**
 * Decode raw image bytes to luma8 grayscale.
 */
async function decodeImage(bytes) {
  if (!isNode) {
    const blob = new Blob([bytes]);
    const bitmap = await createImageBitmap(blob);
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    const ctx = canvas.getContext('2d');
    ctx.drawImage(bitmap, 0, 0);
    const { data } = ctx.getImageData(0, 0, bitmap.width, bitmap.height);
    const pixels = new Uint8Array(bitmap.width * bitmap.height);
    for (let i = 0; i < pixels.length; i++) {
      const j = i * 4;
      pixels[i] = Math.round(0.299 * data[j] + 0.587 * data[j + 1] + 0.114 * data[j + 2]);
    }
    bitmap.close();
    return { width: bitmap.width, height: bitmap.height, pixels };
  }

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
 */
async function loadResources(source) {
  const { fonts, images } = wasm.get_resources(source);
  const resources = { images: {}, fonts: {} };

  await Promise.all([
    ...images.map(async (url) => {
      const resolved = resolveUrl(url);
      // Check in-memory cache
      if (memCache.has(resolved)) {
        resources.images[url] = memCache.get(resolved);
        return;
      }
      const bytes = await loadBytesWithCache(resolved);
      const decoded = await decodeImage(bytes);
      memCache.set(resolved, decoded);
      resources.images[url] = decoded;
    }),
    ...fonts.map(async (url) => {
      const resolved = resolveUrl(url);
      if (memCache.has(resolved)) {
        resources.fonts[url] = memCache.get(resolved);
        return;
      }
      const bytes = await loadBytesWithCache(resolved);
      memCache.set(resolved, bytes);
      resources.fonts[url] = bytes;
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
   * Set global options for resource loading.
   *
   * @param {object} options
   * @param {string} [options.basePath] - Base path for resolving relative resource URLs.
   * @param {string} [options.cachePath] - Directory for disk caching fetched resources (Node.js only).
   */
  static configure(options = {}) {
    if (options.basePath !== undefined) config.basePath = options.basePath;
    if (options.cachePath !== undefined) config.cachePath = options.cachePath;
  }

  /**
   * Clear the in-memory resource cache.
   */
  static clearCache() {
    memCache.clear();
  }

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
