#!/usr/bin/env node
// Static server for the exported Mintlify docs. Drop-in replacement for the
// serve.js shipped by `mint export`, with correct cache headers so browsers
// never get stuck on a stale build:
//   - /_next/static/*  -> content-hashed, cache forever (immutable)
//   - *.html / routes  -> no-cache (+ ETag) so they always revalidate
//   - everything else  -> short cache, must-revalidate
const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = process.env.PORT || 3000;
const DIR = __dirname;

const MIME_TYPES = {
  '.html': 'text/html',
  '.css': 'text/css',
  '.js': 'application/javascript',
  '.json': 'application/json',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.ttf': 'font/ttf',
  '.webp': 'image/webp',
  '.mp4': 'video/mp4',
  '.webm': 'video/webm',
  '.xml': 'application/xml',
  '.txt': 'text/plain',
};

function resolveFile(urlPath) {
  const filePath = path.resolve(DIR, urlPath.replace(/^\/+/, ''));
  if (!filePath.startsWith(DIR + path.sep) && filePath !== DIR) return undefined;
  const candidates = [filePath, path.join(filePath, 'index.html'), filePath + '.html'];
  return candidates.find((c) => fs.existsSync(c) && fs.statSync(c).isFile());
}

function cacheControl(urlPath, ext) {
  if (urlPath.startsWith('/_next/static/')) return 'public, max-age=31536000, immutable';
  if (ext === '.html') return 'no-cache';
  return 'public, max-age=3600, must-revalidate';
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
      'Content-Type': MIME_TYPES[ext] || 'application/octet-stream',
      'Cache-Control': cacheControl(urlPath, ext),
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
    console.log('Serving docs on port ' + PORT);
  });
