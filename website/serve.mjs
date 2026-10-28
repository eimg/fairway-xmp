// Dependency-free static preview. Serve the repository prefix as GitHub Pages does.
import { createServer } from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { resolve, sep } from 'node:path';

const root = fileURLToPath(new URL('./dist/', import.meta.url));
const prefix = '/fairway-xmp/';
const port = Number(process.env.SITE_PORT || 4173);
const types = { '.html': 'text/html; charset=utf-8', '.css': 'text/css; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.png': 'image/png' };
const server = createServer(async (req, res) => {
  const path = new URL(req.url, 'http://localhost').pathname;
  if (path === '/' || path === '/fairway-xmp') {
    res.writeHead(302, { Location: prefix }); res.end(); return;
  }
  if (!path.startsWith(prefix)) { res.writeHead(404); res.end('Not found'); return; }
  try {
    let file = resolve(root, decodeURIComponent(path.slice(prefix.length)));
    if (!file.startsWith(root.endsWith(sep) ? root : root + sep) && file !== resolve(root)) {
      res.writeHead(403); res.end('Forbidden'); return;
    }
    if ((await stat(file)).isDirectory()) {
      if (!path.endsWith('/')) { res.writeHead(301, { Location: path + '/' }); res.end(); return; }
      file = resolve(file, 'index.html');
    }
    const body = await readFile(file);
    res.writeHead(200, { 'Content-Type': types[file.slice(file.lastIndexOf('.'))] || 'application/octet-stream', 'Cache-Control': 'no-store' });
    res.end(body);
  } catch { res.writeHead(404); res.end('Not found'); }
});
server.listen(port, '127.0.0.1', () => console.log(`Local: http://127.0.0.1:${port}${prefix}`));
server.on('error', error => { console.error(error.message); process.exitCode = 1; });
