// Bündelt custom/workflow.ts mit dem esbuild des Clones und gibt den Default-Export als JSON aus.
// Aufruf: node workflow_load.mjs <clone> <datei> <core>; Fehler erscheinen einzeilig auf stderr.
// <core> ist der Ersatz für @harness/core, den der Fachkern daneben geschrieben hat.
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

const [cloneDir, file, core] = process.argv.slice(2);

try {
  const require = createRequire(join(cloneDir, 'package.json'));
  const esbuild = require('esbuild');
  // @harness/core zeigt auf den Ersatz aus dem Fachkern (Plugin statt alias, weil alias keine absoluten Pfade nimmt)
  const out = await esbuild.build({
    entryPoints: [file], bundle: true, platform: 'node', format: 'esm', write: false, logLevel: 'silent',
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
