import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { join, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const wasmDir = join(__dirname, '..');

const MIME = {
  '.html': 'text/html',
  '.js':   'application/javascript',
  '.mjs':  'application/javascript',
  '.wasm': 'application/wasm',
  '.css':  'text/css',
  '.json': 'application/json',
  '.ts':   'application/typescript',
};

const server = createServer(async (req, res) => {
  let path = req.url.split('?')[0];
  if (path === '/') path = '/browser_demo/index.html';

  const filePath = join(wasmDir, path);
  const ext = extname(filePath);

  try {
    const data = await readFile(filePath);
    res.writeHead(200, {
      'Content-Type': MIME[ext] || 'application/octet-stream',
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp',
    });
    res.end(data);
  } catch {
    res.writeHead(404);
    res.end('Not found');
  }
});

const port = 3333;
server.listen(port, () => {
  console.log(`Rip demo: http://localhost:${port}`);
});
