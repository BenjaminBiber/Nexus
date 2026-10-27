// Gemeinsame esbuild-Einstellungen für die Node-Bündel des Harness: das Plugin (packages/plugin/scripts/build-plugin.mjs)
// bündelt nach ESM, das Sidecar (scripts/sidecar/build-sidecar.mjs) nach CommonJS, weil eine Single Executable
// Application (SEA) nur CommonJS als Einstieg kennt. Sonst sind die Einstellungen gleich.
import { build } from 'esbuild';

// Hilfsfunktion `require` für gebündelte CommonJS-Abhängigkeiten (z. B. Fastify) im ESM-Ergebnis
const ESM_BANNER = "import { createRequire as __createRequire } from 'node:module';\nconst require = __createRequire(import.meta.url);";

// Ersatz für import.meta.url im CommonJS-Ergebnis: dort gibt es import.meta nicht, wohl aber __filename
export const IMPORT_META_URL = '__harnessImportMetaUrl';

// Im CommonJS-Ergebnis ist `require` schon da; stattdessen braucht es die Ersatzvariable
const CJS_BANNER = `const ${IMPORT_META_URL} = require('node:url').pathToFileURL(__filename).href;`;

// Bündelt eine Programmdatei zu einer einzelnen Datei für Node (Pfade absolut)
export async function bundleForNode({ entry, outfile, format = 'esm', define = {} }) {
  await build({
    entryPoints: [entry],
    outfile,
    bundle: true,
    platform: 'node',
    format,
    target: 'node22',
    banner: { js: format === 'cjs' ? CJS_BANNER : ESM_BANNER },
    define: format === 'cjs' ? { 'import.meta.url': IMPORT_META_URL, ...define } : define,
    logLevel: 'warning',
  });
}
