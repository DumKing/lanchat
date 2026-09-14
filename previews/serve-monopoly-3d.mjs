import { createServer } from 'vite';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const server = await createServer({
  root: fileURLToPath(new URL('../', import.meta.url)),
  configFile: false,
  cacheDir: join(tmpdir(), 'lanchat-monopoly-3d-vite'),
  server: { host: '127.0.0.1', port: 1426, strictPort: true },
});
await server.listen();
server.printUrls();
