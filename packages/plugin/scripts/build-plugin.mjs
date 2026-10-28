// Baut das eigenständige Plugin: bündelt CLI und Server mit esbuild, kopiert die Oberfläche und das Manifest.
// Voraussetzung: `npm run build` (erzeugt packages/*/dist). Ergebnis: packages/plugin/dist/ai-harness/
import { chmodSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { bundleForNode } from '../../../scripts/nodeBundle.mjs';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
// Zielordner: Vorgabe im Repo, für Testläufe über HARNESS_PLUGIN_OUT auf einen eigenen Ordner umstellbar
const OUT = process.env.HARNESS_PLUGIN_OUT ? resolve(process.env.HARNESS_PLUGIN_OUT) : join(ROOT, 'packages/plugin/dist/ai-harness');

// Prüft, ob alle Eingaben gebaut sind
function assertInputs() {
  for (const file of ['packages/cli/dist/main.js', 'packages/server/dist/main.js', 'packages/web/dist/index.html', 'packages/core/dist/index.js']) {
    if (!existsSync(join(ROOT, file))) {
      console.error(`Fehlt: ${file}. Bitte zuerst "npm run build" ausführen.`);
      process.exit(1);
    }
  }
}

// Bündelt eine Programmdatei zu einer einzelnen ESM-Datei
async function bundle(entry, outfile) {
  await bundleForNode({ entry: join(ROOT, entry), outfile: join(OUT, outfile) });
}

assertInputs();
rmSync(OUT, { recursive: true, force: true });
mkdirSync(OUT, { recursive: true });
cpSync(join(ROOT, 'packages/plugin/manifest'), OUT, { recursive: true });
await bundle('packages/cli/dist/main.js', 'lib/cli.mjs');
await bundle('packages/server/dist/main.js', 'lib/server.mjs');
cpSync(join(ROOT, 'packages/web/dist'), join(OUT, 'web'), { recursive: true });
cpSync(join(ROOT, 'packages/server/assets/toast'), join(OUT, 'toast'), { recursive: true });
chmodSync(join(OUT, 'bin/harness'), 0o755);

// Version aus dem Serverpaket; die dritte Stelle ist ein Build-Zeitstempel (Sekunden), damit
// `claude plugin update` jeden neuen Build als neuere Version erkennt und den Plugin-Cache erneuert
const [major, minor] = JSON.parse(readFileSync(join(ROOT, 'packages/server/package.json'), 'utf8')).version.split('.');
const version = `${major}.${minor}.${Math.floor(Date.now() / 1000)}`;
const manifestPath = join(OUT, '.claude-plugin/plugin.json');
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
manifest.version = version;
writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + '\n');

// Schreibt eine Übersicht mit Rückfall auf den Standard, wenn das Anwenden der Anpassungen selbst scheitert (ohne workflow.json)
function writeFallbackOverview(customDir, message) {
  const dir = join(OUT, 'customizations');
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  const problem = { code: 'apply-failed', message: `Anpassungen ließen sich nicht anwenden: ${message}` };
  const entry = { point: 'workflow', name: 'workflow', kind: 'replace', state: 'fallback', base: 'none', problems: [problem], warnings: [] };
  const overview = { version: 1, createdAt: new Date().toISOString(), cloneDir: ROOT, customDir, harnessVersion: version, workflow: 'fallback', entries: [entry] };
  writeFileSync(join(dir, 'overview.json'), JSON.stringify(overview, null, 2) + '\n');
}

// Anpassungen des Devs (custom/) prüfen und über den Standard legen; Ordner für Tests über HARNESS_CUSTOM_DIR
const customDir = process.env.HARNESS_CUSTOM_DIR || join(ROOT, 'custom');
try {
  const core = await import(pathToFileURL(join(ROOT, 'packages/core/dist/index.js')).href);
  await new core.CustomizationApplier().apply(
    { cloneDir: ROOT, customDir, standardDir: OUT, harnessVersion: version, tools: new core.CloneWorkflowTools(ROOT) },
    OUT,
  );
} catch (error) {
  // Eine Anpassung darf den Build nie abbrechen (FR-015): Hinweis ausgeben, Übersicht mit Rückfall schreiben, weiterbauen
  const message = error instanceof Error ? error.message : String(error);
  console.warn(`Anpassungen ließen sich nicht anwenden: ${message}. Es gilt der Standard.`);
  writeFallbackOverview(customDir, message);
}

console.log(`Plugin gebaut: ${OUT}`);
