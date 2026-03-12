/**
 * Smoke test — exercises the native Node.js addon end-to-end.
 *
 * Run: node rip_nodejs/test_smoke.mjs
 */

import { renderHtml, renderText, renderImage, renderRaster, renderEscpos } from './index.js';

let passed = 0;
let failed = 0;

function assert(condition, message) {
    if (!condition) {
        console.error(`  \u2717 ${message}`);
        failed++;
    } else {
        console.log(`  \u2713 ${message}`);
        passed++;
    }
}

// ─── renderHtml ─────────────────────────────────────────────────────

console.log('\nrenderHtml:');

const html = await renderHtml('## Hello World\n---\n| Coffee | $4.50 |');
assert(typeof html === 'string', 'returns a string');
assert(html.includes('Hello World'), 'contains title text');
assert(html.includes('Coffee'), 'contains column text');
assert(html.includes('$4.50'), 'contains price');
assert(html.includes('<html'), 'is a full HTML document');

// ─── renderText ─────────────────────────────────────────────────────

console.log('\nrenderText:');

const text = await renderText('## Hello World\n---\n| Coffee | $4.50 |');
assert(typeof text === 'string', 'returns a string');
assert(text.includes('Hello World'), 'contains title text');
assert(text.includes('Coffee'), 'contains column text');
assert(text.includes('$4.50'), 'contains price');

// ─── renderImage ────────────────────────────────────────────────────

console.log('\nrenderImage:');

const png = await renderImage('Hello');
assert(png instanceof Buffer, 'returns a Buffer');
assert(png.length > 0, 'non-empty output');
// PNG signature: 137 80 78 71 13 10 26 10
assert(
    png[0] === 0x89 && png[1] === 0x50 && png[2] === 0x4E && png[3] === 0x47,
    'starts with PNG signature'
);

// ─── renderRaster ───────────────────────────────────────────────────

console.log('\nrenderRaster:');

const raster = await renderRaster('Hello');
assert(raster instanceof Buffer, 'returns a Buffer');
assert(raster.length > 0, 'non-empty output');
// ESC/POS raster starts with ESC @ (init)
assert(raster[0] === 0x1B && raster[1] === 0x40, 'starts with ESC @ (init)');

// ─── renderEscpos ───────────────────────────────────────────────────

console.log('\nrenderEscpos:');

const escpos = await renderEscpos('Hello\n@cut()');
assert(escpos instanceof Buffer, 'returns a Buffer');
assert(escpos.length > 0, 'non-empty output');
assert(escpos[0] === 0x1B && escpos[1] === 0x40, 'starts with ESC @ (init)');

// ─── Timing ─────────────────────────────────────────────────────────

console.log('\nPerformance:');

const start = performance.now();
await renderHtml('## Hello\n---\nItem | $5.00');
const elapsed = performance.now() - start;
assert(elapsed < 50, `renderHtml is fast (${elapsed.toFixed(1)}ms < 50ms)`);

// ─── Summary ────────────────────────────────────────────────────────

console.log(`\n${passed + failed} tests: ${passed} passed, ${failed} failed\n`);
process.exit(failed > 0 ? 1 : 0);
