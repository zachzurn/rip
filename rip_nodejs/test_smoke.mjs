/**
 * Smoke test — exercises the native Node.js addon end-to-end.
 *
 * Run: node rip_nodejs/test_smoke.mjs
 */

import {
    parse,
    resolveResources,
    renderHtml,
    renderText,
    renderImage,
    renderRaster,
    renderEscpos,
    renderHtmlFromMarkup,
    renderTextFromMarkup,
    renderImageFromMarkup,
    renderRasterFromMarkup,
    renderEscposFromMarkup,
} from './index.js';

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

// ─── parse + Document ──────────────────────────────────────────────

console.log('\nparse:');

const doc = parse('## Hello World\n---\n| Coffee | $4.50 |');
assert(doc != null, 'returns a Document object');

// ─── resolveResources ──────────────────────────────────────────────

console.log('\nresolveResources:');

const needed = resolveResources(doc);
assert(Array.isArray(needed), 'returns an array');
assert(needed.length === 0, 'no remote resources needed for local-only markup');

// ─── renderHtml (Document) ─────────────────────────────────────────

console.log('\nrenderHtml (Document):');

const html = await renderHtml(doc);
assert(typeof html === 'string', 'returns a string');
assert(html.includes('Hello World'), 'contains title text');
assert(html.includes('Coffee'), 'contains column text');
assert(html.includes('$4.50'), 'contains price');
assert(html.includes('<html'), 'is a full HTML document');

// ─── renderHtmlFromMarkup (convenience) ────────────────────────────

console.log('\nrenderHtmlFromMarkup:');

const html2 = await renderHtmlFromMarkup('## Hello World\n---\n| Coffee | $4.50 |');
assert(typeof html2 === 'string', 'returns a string');
assert(html2.includes('Hello World'), 'contains title text');

// ─── renderText (Document) ─────────────────────────────────────────

console.log('\nrenderText (Document):');

const text = await renderText(doc);
assert(typeof text === 'string', 'returns a string');
assert(text.includes('Hello World'), 'contains title text');
assert(text.includes('Coffee'), 'contains column text');
assert(text.includes('$4.50'), 'contains price');

// ─── renderTextFromMarkup (convenience) ────────────────────────────

console.log('\nrenderTextFromMarkup:');

const text2 = await renderTextFromMarkup('## Hello\nWorld');
assert(typeof text2 === 'string', 'returns a string');
assert(text2.includes('Hello'), 'contains text');

// ─── renderImage (Document) ────────────────────────────────────────

console.log('\nrenderImage (Document):');

const png = await renderImage(doc);
assert(png instanceof Buffer, 'returns a Buffer');
assert(png.length > 0, 'non-empty output');
// PNG signature: 137 80 78 71 13 10 26 10
assert(
    png[0] === 0x89 && png[1] === 0x50 && png[2] === 0x4E && png[3] === 0x47,
    'starts with PNG signature'
);

// ─── renderImageFromMarkup (convenience) ───────────────────────────

console.log('\nrenderImageFromMarkup:');

const png2 = await renderImageFromMarkup('Hello');
assert(png2 instanceof Buffer, 'returns a Buffer');
assert(png2[0] === 0x89 && png2[1] === 0x50, 'valid PNG signature');

// ─── renderRaster (Document) ───────────────────────────────────────

console.log('\nrenderRaster (Document):');

const raster = await renderRaster(doc);
assert(raster instanceof Buffer, 'returns a Buffer');
assert(raster.length > 0, 'non-empty output');
// ESC/POS raster starts with ESC @ (init)
assert(raster[0] === 0x1B && raster[1] === 0x40, 'starts with ESC @ (init)');

// ─── renderRasterFromMarkup (convenience) ──────────────────────────

console.log('\nrenderRasterFromMarkup:');

const raster2 = await renderRasterFromMarkup('Hello');
assert(raster2 instanceof Buffer, 'returns a Buffer');
assert(raster2[0] === 0x1B && raster2[1] === 0x40, 'starts with ESC @ (init)');

// ─── renderEscpos (Document) ───────────────────────────────────────

console.log('\nrenderEscpos (Document):');

const escpos = await renderEscpos(doc);
assert(escpos instanceof Buffer, 'returns a Buffer');
assert(escpos.length > 0, 'non-empty output');
assert(escpos[0] === 0x1B && escpos[1] === 0x40, 'starts with ESC @ (init)');

// ─── renderEscposFromMarkup (convenience) ──────────────────────────

console.log('\nrenderEscposFromMarkup:');

const escpos2 = await renderEscposFromMarkup('Hello\n@cut()');
assert(escpos2 instanceof Buffer, 'returns a Buffer');
assert(escpos2[0] === 0x1B && escpos2[1] === 0x40, 'starts with ESC @ (init)');

// ─── Timing ─────────────────────────────────────────────────────────

console.log('\nPerformance:');

const start = performance.now();
await renderHtml(parse('## Hello\n---\nItem | $5.00'));
const elapsed = performance.now() - start;
assert(elapsed < 50, `renderHtml is fast (${elapsed.toFixed(1)}ms < 50ms)`);

// ─── Summary ────────────────────────────────────────────────────────

console.log(`\n${passed + failed} tests: ${passed} passed, ${failed} failed\n`);
process.exit(failed > 0 ? 1 : 0);
