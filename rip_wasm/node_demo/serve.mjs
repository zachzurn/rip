/**
 * Node.js server demo — renders markup server-side via POST.
 *
 * The browser sends { source, mode } and the server returns the
 * rendered result using the Rip WASM library running in Node.js.
 *
 * Run:  node rip_wasm/node_demo/serve.mjs
 */

import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Rip } from '../pkg/rip.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));

function readBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    req.on('data', c => chunks.push(c));
    req.on('end', () => resolve(Buffer.concat(chunks).toString()));
    req.on('error', reject);
  });
}

const server = createServer(async (req, res) => {
  // Serve the frontend
  if (req.method === 'GET' && (req.url === '/' || req.url === '/index.html')) {
    const html = await readFile(join(__dirname, 'index.html'));
    res.writeHead(200, { 'Content-Type': 'text/html' });
    res.end(html);
    return;
  }

  // Render endpoint
  if (req.method === 'POST' && req.url === '/render') {
    let body;
    try {
      body = JSON.parse(await readBody(req));
    } catch {
      res.writeHead(400, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: 'Invalid JSON' }));
      return;
    }
    const { source, mode } = body;
    const start = performance.now();

    try {
      let result;

      if (mode === 'html') {
        result = JSON.stringify({ html: await Rip.renderHtml(source) });
      } else if (mode === 'text') {
        result = JSON.stringify({ text: await Rip.renderText(source) });
      } else if (mode === 'pixels') {
        const { width, height, pixels } = await Rip.renderPixels(source);
        const b64 = Buffer.from(pixels).toString('base64');
        result = JSON.stringify({ width, height, pixels: b64 });
      } else {
        res.writeHead(400);
        res.end(JSON.stringify({ error: `Unknown mode: ${mode}` }));
        return;
      }

      const elapsed = (performance.now() - start).toFixed(1);
      res.writeHead(200, {
        'Content-Type': 'application/json',
        'X-Render-Time': `${elapsed}ms`,
      });
      res.end(result);
    } catch (e) {
      res.writeHead(500, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: e.message || String(e) }));
    }
    return;
  }

  res.writeHead(404);
  res.end('Not found');
});

const port = 3334;
server.listen(port, () => {
  console.log(`Rip node demo: http://localhost:${port}`);
});
