#!/usr/bin/env node
// Zero-dependency static server for the HealthKite landing page.
// Every response is `no-cache` + ETag so browsers always revalidate and never
// get stuck on a stale build (assets here are not content-hashed). 304s keep it cheap.
const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = process.env.PORT || 3000;
const DIR = __dirname;

const MIME = {
  '.html': 'text/html',
  '.css': 'text/css',
  '.js': 'application/javascript',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.webp': 'image/webp',
  '.ico': 'image/x-icon',
  '.woff2': 'font/woff2',
  '.woff': 'font/woff',
  '.txt': 'text/plain',
  '.xml': 'application/xml',
  '.mp4': 'video/mp4',
};

function resolveFile(urlPath) {
  const fp = path.resolve(DIR, urlPath.replace(/^\/+/, ''));
  if (!fp.startsWith(DIR + path.sep) && fp !== DIR) return undefined;
  const candidates = [fp, path.join(fp, 'index.html'), fp + '.html'];
  return candidates.find((c) => fs.existsSync(c) && fs.statSync(c).isFile());
}

http
  .createServer((req, res) => {
    let urlPath;
    try {
      urlPath = decodeURIComponent((req.url || '/').split('?')[0]);
    } catch {
      res.writeHead(404, { 'Content-Type': 'text/plain', 'Cache-Control': 'no-cache' });
      res.end('404 Not Found');
      return;
    }

    const file = resolveFile(urlPath);
    if (!file) {
      res.writeHead(404, { 'Content-Type': 'text/plain', 'Cache-Control': 'no-cache' });
      res.end('404 Not Found');
      return;
    }

    const ext = path.extname(file).toLowerCase();
    const st = fs.statSync(file);
    const etag = '"' + st.size.toString(16) + '-' + Math.round(st.mtimeMs).toString(16) + '"';
    const headers = {
      'Content-Type': MIME[ext] || 'application/octet-stream',
      'Cache-Control': 'no-cache',
      ETag: etag,
    };

    if (req.headers['if-none-match'] === etag) {
      res.writeHead(304, headers);
      res.end();
      return;
    }

    res.writeHead(200, headers);
    if (req.method === 'HEAD') {
      res.end();
      return;
    }
    fs.createReadStream(file).pipe(res);
  })
  .listen(PORT, '0.0.0.0', () => {
    console.log('Serving landing on port ' + PORT);
  });
