import { createReadStream, statSync } from 'node:fs';
import { createServer } from 'node:http';
import { dirname, extname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '..');
const port = Number(process.env.CODEX_MONITOR_PORT || 4173);
const mimeTypes = {
  '.css': 'text/css; charset=utf-8',
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml'
};

function resolveRequestPath(requestUrl) {
  const pathname = decodeURIComponent(new URL(requestUrl, 'http://127.0.0.1').pathname);
  const filePath = resolve(root, `.${pathname === '/' ? '/index.html' : pathname}`);
  return filePath.startsWith(root) ? filePath : null;
}

const server = createServer((request, response) => {
  const filePath = resolveRequestPath(request.url || '/');
  if (!filePath) {
    response.writeHead(403).end('Forbidden');
    return;
  }

  try {
    const stat = statSync(filePath);
    if (!stat.isFile()) throw new Error('Not a file');
    response.writeHead(200, { 'content-type': mimeTypes[extname(filePath)] || 'application/octet-stream' });
    createReadStream(filePath).pipe(response);
  } catch {
    response.writeHead(404).end('Not found');
  }
});

server.listen(port, '127.0.0.1', () => {
  console.log(`Token Monitor is available at http://127.0.0.1:${port}`);
});

function shutdown() {
  server.close(() => process.exit(0));
}

process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);
