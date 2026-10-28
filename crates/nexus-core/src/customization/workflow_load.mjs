// Bündelt custom/workflow.ts mit dem esbuild des Clones und gibt den Default-Export als JSON aus.
// Aufruf: node workflow_load.mjs <clone> <datei>; Fehler erscheinen einzeilig auf stderr.
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

const [cloneDir, file] = process.argv.slice(2);

try {
  const require = createRequire(join(cloneDir, 'package.json'));
  const esbuild = require('esbuild');
  // @harness/core zeigt auf das gebaute core-Paket des Clones (Plugin statt alias, weil alias keine absoluten Pfade nimmt)
  const core = join(cloneDir, 'packages', 'core', 'dist', 'index.js');
  // Hilfsfunktion `require` für gebündelte CommonJS-Abhängigkeiten von core (z. B. yaml); ohne sie scheitert der Import in reinem Node
  const banner = "import { createRequire as __createRequire } from 'node:module';\nconst require = __createRequire(import.meta.url);";
  const out = await esbuild.build({
    entryPoints: [file], bundle: true, platform: 'node', format: 'esm', write: false, logLevel: 'silent', banner: { js: banner },
    plugins: [{ name: 'harness-core', setup: (b) => b.onResolve({ filter: /^@harness\/core$/ }, () => ({ path: core })) }],
  });
  // Gebündelten Code in eine temporäre Datei schreiben und importieren; JSON entfernt Funktionen
  const dir = mkdtempSync(join(tmpdir(), 'harness-workflow-load-'));
  try {
    const target = join(dir, 'workflow.mjs');
    writeFileSync(target, out.outputFiles[0].text);
    const module = await import(pathToFileURL(target).href);
    process.stdout.write(JSON.stringify(module.default ?? null));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(message.split(/\r?\n/).join(' '));
  process.exit(1);
}
