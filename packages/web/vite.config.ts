import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { promisify } from 'node:util';
import { brotliCompress, constants, gzip } from 'node:zlib';
import { defineConfig, type Plugin } from 'vite';
import react from '@vitejs/plugin-react';
import { hostModules } from './vite/hostModules.js';

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

// Entwicklung: /api und /themes gehen an den laufenden Harness-Server (Standardport 4710, starten mit npm run dev:server)
const DEV_SERVER = 'http://127.0.0.1:4710';
// Host und Herkunft werden auf die Serveradresse gesetzt, sonst lehnt der Schutz des Servers die Anfragen ab
const proxied = { target: DEV_SERVER, changeOrigin: true, headers: { origin: DEV_SERVER } };

export default defineConfig({
  plugins: [react(), hostModules(), precompress()],
  server: {
    // /themes liefert Manifest, main.js und Dateien installierter Themes; ohne Weiterleitung fängt die SPA-Rückfallseite sie ab
    proxy: { '/api': proxied, '/themes': proxied },
  },
  build: { outDir: 'dist', chunkSizeWarningLimit: 1500 },
});
