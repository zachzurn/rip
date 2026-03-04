/**
 * Smoke test — exercises the JS wrapper (rip.mjs) end-to-end in Node.js.
 *
 * Run: node rip_wasm/test_smoke.mjs
 */

import { Rip } from './pkg/rip.mjs';

let passed = 0;
let failed = 0;

function assert(condition, message) {
  if (!condition) {
    console.error(`  ✗ ${message}`);
    failed++;
  } else {
    console.log(`  ✓ ${message}`);
    passed++;
  }
}

// ─── renderHtml ─────────────────────────────────────────────────────

console.log('\nrenderHtml:');

const html = await Rip.renderHtml('## Hello World\n---\n| Coffee | $4.50 |');
assert(typeof html === 'string', 'returns a string');
assert(html.includes('Hello World'), 'contains title text');
assert(html.includes('Coffee'), 'contains column text');
assert(html.includes('$4.50'), 'contains price');
assert(html.includes('<html'), 'is a full HTML document');

// ─── renderText ─────────────────────────────────────────────────────

console.log('\nrenderText:');

const text = await Rip.renderText('## Hello World\n---\n| Coffee | $4.50 |');
assert(typeof text === 'string', 'returns a string');
assert(text.includes('Hello World'), 'contains title text');
assert(text.includes('Coffee'), 'contains column text');
assert(text.includes('$4.50'), 'contains price');

// ─── renderPixels ───────────────────────────────────────────────────

console.log('\nrenderPixels:');

const pixels = await Rip.renderPixels('Hello');
assert(typeof pixels === 'object', 'returns an object');
assert(typeof pixels.width === 'number' && pixels.width > 0, 'has positive width');
assert(typeof pixels.height === 'number' && pixels.height > 0, 'has positive height');
assert(pixels.pixels instanceof Uint8Array, 'pixels is Uint8Array');
assert(pixels.pixels.length === pixels.width * pixels.height, 'pixel count = width × height');
assert(Array.isArray(pixels.dirtyRows), 'dirtyRows is an array');
assert(pixels.dirtyRows.length === pixels.height, 'dirtyRows length = height');

// ─── renderRaster ───────────────────────────────────────────────────

console.log('\nrenderRaster:');

const raster = await Rip.renderRaster('Hello');
assert(typeof raster === 'object', 'returns an object');
assert(raster.pixels instanceof Uint8Array, 'pixels is Uint8Array');
const expectedLen = Math.ceil(raster.width / 8) * raster.height;
assert(raster.pixels.length === expectedLen, `packed pixel length = ceil(w/8) × h (${raster.pixels.length} === ${expectedLen})`);

// ─── renderEscpos ───────────────────────────────────────────────────

console.log('\nrenderEscpos:');

const escpos = await Rip.renderEscpos('Hello\n@cut()');
assert(escpos instanceof Uint8Array, 'returns Uint8Array');
assert(escpos.length > 0, 'non-empty output');
assert(escpos[0] === 0x1B && escpos[1] === 0x40, 'starts with ESC @ (init)');

// ─── Lazy init (second call should be fast) ─────────────────────────

console.log('\nLazy init:');

const start = performance.now();
const html2 = await Rip.renderHtml('test');
const elapsed = performance.now() - start;
assert(elapsed < 50, `second call is fast (${elapsed.toFixed(1)}ms < 50ms)`);

// ─── Summary ────────────────────────────────────────────────────────

console.log(`\n${passed + failed} tests: ${passed} passed, ${failed} failed\n`);
process.exit(failed > 0 ? 1 : 0);
