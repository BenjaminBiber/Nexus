// Baut das eigenständige Plugin: baut das Rust-Binary `nexus`, bündelt den Node-Server mit esbuild,
// kopiert die Oberfläche und das Manifest.
// Voraussetzung: `npm run build` (erzeugt packages/*/dist). Ergebnis: packages/plugin/dist/nexus/
import { chmodSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { bundleForNode } from '../../../scripts/nodeBundle.mjs';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
// Zielordner: Vorgabe im Repo, für Testläufe über NEXUS_PLUGIN_OUT (alt: HARNESS_PLUGIN_OUT) umstellbar
const OUT_ENV = process.env.NEXUS_PLUGIN_OUT ?? process.env.HARNESS_PLUGIN_OUT;
const OUT = OUT_ENV ? resolve(OUT_ENV) : join(ROOT, 'packages/plugin/dist/nexus');
// Zielplattform des Binaries (für den Release-Workflow); ohne sie baut cargo für diesen Rechner
const TARGET = process.env.NEXUS_TARGET;
// Name des Binaries im Plugin; unter Windows mit Endung
const BINARY = process.platform === 'win32' ? 'nexus.exe' : 'nexus';

// Prüft, ob alle Eingaben gebaut sind
function assertInputs() {
  for (const file of ['packages/server/dist/main.js', 'packages/web/dist/index.html']) {
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

// Baut das Rust-Binary und legt es als bin/nexus[.exe] ins Plugin
function buildBinary() {
  const args = ['build', '--release', '-p', 'nexus-cli', '--bin', 'nexus'];
  if (TARGET) args.push('--target', TARGET);
  const result = spawnSync('cargo', args, { cwd: ROOT, stdio: 'inherit', windowsHide: true, shell: process.platform === 'win32' });
  if (result.status !== 0) {
    console.error('Fehlgeschlagen: cargo build --release -p nexus-cli. Die Meldung steht oben.');
    process.exit(1);
  }
  const built = join(ROOT, 'target', ...(TARGET ? [TARGET] : []), 'release', BINARY);
  if (!existsSync(built)) {
    console.error(`Fehlt nach dem Bauen: ${built}`);
    process.exit(1);
  }
  mkdirSync(join(OUT, 'bin'), { recursive: true });
  cpSync(built, join(OUT, 'bin', BINARY));
  chmodSync(join(OUT, 'bin', BINARY), 0o755);
}

assertInputs();
rmSync(OUT, { recursive: true, force: true });
mkdirSync(OUT, { recursive: true });
cpSync(join(ROOT, 'packages/plugin/manifest'), OUT, { recursive: true });
buildBinary();
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

// Schreibt eine Übersicht mit Rückfall auf den Standard, wenn das Binary selbst nicht läuft (ohne workflow.json)
function writeFallbackOverview(customDir, message) {
  const dir = join(OUT, 'customizations');
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  const problem = { code: 'apply-failed', message: `Anpassungen ließen sich nicht anwenden: ${message}` };
  const entry = { point: 'workflow', name: 'workflow', kind: 'replace', state: 'fallback', base: 'none', problems: [problem], warnings: [] };
  const overview = { version: 1, createdAt: new Date().toISOString(), cloneDir: ROOT, customDir, harnessVersion: version, workflow: 'fallback', entries: [entry] };
  writeFileSync(join(dir, 'overview.json'), JSON.stringify(overview, null, 2) + '\n');
}

// Anpassungen des Devs (custom/) prüfen und über den Standard legen; Ordner für Tests über NEXUS_CUSTOM_DIR
const customDir = process.env.NEXUS_CUSTOM_DIR || process.env.HARNESS_CUSTOM_DIR || join(ROOT, 'custom');
const applied = spawnSync(
  join(OUT, 'bin', BINARY),
  ['customize', '--clone', ROOT, '--custom', customDir, 'apply', '--standard', OUT, '--out', OUT, '--version', version],
  {
    cwd: ROOT,
    stdio: 'inherit',
    windowsHide: true,
    // Eigener (nie angelegter) Benutzerordner: `customize apply` liest ihn nicht, und ein Build
    // darf nie den Umzug der Daten des Devs nach ~/.nexus auslösen
    env: { ...process.env, NEXUS_HOME: join(ROOT, 'target', 'plugin-build-home') },
  },
);
if (applied.status !== 0) {
  // Eine Anpassung darf den Build nie abbrechen (FR-015): Hinweis ausgeben, Übersicht mit Rückfall schreiben, weiterbauen
  const message = applied.error ? applied.error.message : `nexus customize apply endete mit Code ${applied.status}`;
  console.warn(`Anpassungen ließen sich nicht anwenden: ${message}. Es gilt der Standard.`);
  writeFallbackOverview(customDir, message);
}

console.log(`Plugin gebaut: ${OUT}`);
