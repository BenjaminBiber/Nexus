import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { promisify } from 'node:util';
import { brotliCompress, constants, gzip } from 'node:zlib';
import { defineConfig, type Plugin } from 'vite';
import react from '@vitejs/plugin-react';

const brotli = promisify(brotliCompress);
const gzipAsync = promisify(gzip);

// Legt neben jede größere JS-/CSS-/SVG-Datei des Builds eine .br- und .gz-Fassung; der Server liefert sie aus (preCompressed)
function precompress(): Plugin {
  return {
    name: 'harness-precompress',
    apply: 'build',
    async writeBundle(options, bundle) {
      const dir = options.dir ?? 'dist';
      await Promise.all(Object.values(bundle).map(async (file) => {
        if (!/\.(js|css|svg|json)$/.test(file.fileName)) return;
        const source = file.type === 'chunk' ? file.code : file.source;
        const data = typeof source === 'string' ? Buffer.from(source) : Buffer.from(source);
        if (data.length < 1024) return;
        const path = join(dir, file.fileName);
        await writeFile(`${path}.br`, await brotli(data, { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } }));
        await writeFile(`${path}.gz`, await gzipAsync(data, { level: 9 }));
      }));
    },
  };
}

// Entwicklung: /api geht an den laufenden Harness-Server (Standardport 4710, starten mit npm run dev:server)
export default defineConfig({
  plugins: [react(), precompress()],
  server: {
    // Host und Herkunft werden auf die Serveradresse gesetzt, sonst lehnt der Schutz des Servers die Anfragen ab
    proxy: { '/api': { target: 'http://127.0.0.1:4710', changeOrigin: true, headers: { origin: 'http://127.0.0.1:4710' } } },
  },
  build: { outDir: 'dist', chunkSizeWarningLimit: 1500 },
});
