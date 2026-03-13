/**
 * Test resource loading — local images, caching, and resolveResources.
 */

import {
    parse,
    resolveResources,
    renderImage,
    renderEscpos,
    renderImageFromMarkup,
} from './index.js';
import { readFileSync, rmSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const examplesDir = join(__dirname, '..', 'examples');
const cacheDir = join(__dirname, 'test-cache');

// Clean cache
try { rmSync(cacheDir, { recursive: true }); } catch {}

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

// ─── parse + resolveResources (local only) ──────────────────────────

console.log('\nresolveResources (local markup):');

const source = readFileSync(join(examplesDir, 'burger-barn.rip'), 'utf-8');
const doc = parse(source);

const needed = resolveResources(doc, {
    resourceDir: examplesDir,
    cacheDir: cacheDir,
});
assert(Array.isArray(needed), 'returns an array');
// burger-barn.rip uses local images, not remote URLs
assert(needed.length === 0, `no remote URLs needed (got ${needed.length})`);

// ─── Local image rendering (Document API) ───────────────────────────

console.log('\nLocal image (burger-barn, Document API):');

const png = await renderImage(doc, {
    resourceDir: examplesDir,
    cacheDir: cacheDir,
});

assert(png instanceof Buffer, 'returns a Buffer');
assert(png.length > 1000, `non-trivial PNG output (${png.length} bytes)`);
assert(png[0] === 0x89 && png[1] === 0x50, 'valid PNG signature');

// ─── Cache was populated ────────────────────────────────────────────

console.log('\nCache:');

const cacheFiles = readdirSync(cacheDir);
assert(cacheFiles.length > 0, `cache dir has files (${cacheFiles.length})`);
console.log(`  cached files: ${cacheFiles.join(', ')}`);

// ─── Second render uses cache (should be faster) ────────────────────

console.log('\nCached render:');

const start = performance.now();
const png2 = await renderImage(doc, {
    resourceDir: examplesDir,
    cacheDir: cacheDir,
});
const elapsed = performance.now() - start;

assert(png2.length === png.length, `same output size (${png2.length} === ${png.length})`);
console.log(`  cached render time: ${elapsed.toFixed(1)}ms`);

// ─── ESC/POS with resources ─────────────────────────────────────────

console.log('\nESC/POS with resources (Document API):');

const escpos = await renderEscpos(doc, {
    resourceDir: examplesDir,
});
assert(escpos instanceof Buffer, 'returns a Buffer');
assert(escpos.length > 100, `non-trivial ESC/POS output (${escpos.length} bytes)`);
assert(escpos[0] === 0x1B && escpos[1] === 0x40, 'starts with ESC @ (init)');

// ─── FromMarkup convenience (backwards compat) ─────────────────────

console.log('\nrenderImageFromMarkup (convenience):');

const png3 = await renderImageFromMarkup(source, {
    resourceDir: examplesDir,
    cacheDir: cacheDir,
});
assert(png3 instanceof Buffer, 'returns a Buffer');
assert(png3.length === png.length, `same output as Document API (${png3.length} === ${png.length})`);

// ─── Cleanup ────────────────────────────────────────────────────────

try { rmSync(cacheDir, { recursive: true }); } catch {}

console.log(`\n${passed + failed} tests: ${passed} passed, ${failed} failed\n`);
process.exit(failed > 0 ? 1 : 0);
