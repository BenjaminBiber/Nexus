// Hängt die Installer eines Ziels und sein CLI-Archiv unter den erwarteten Namen an das Entwurfs-Release.
// Ersetzt den Upload von tauri-action für Builds außerhalb von GitHub Actions (Gitea Actions).
// Voraussetzung: `tauri build --target <ziel> --config src-tauri/tauri.release.conf.json` und cli-archive.mjs.
// Aufruf: node scripts/release/upload-bundles.mjs --version <X.Y.Z> --target <rust-ziel> --tag <vX.Y.Z> [--dry-run]
//   GH_TOKEN und GH_REPO braucht gh für den Upload; mit --dry-run wird nur geprüft, ob alle Dateien da sind.
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { releaseFiles } from './lib/BundleAssets.mjs';
import { updaterEnabled } from './lib/ExpectedAssets.mjs';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

function fail(message) {
  console.error(message);
  process.exit(1);
}

function options(argv) {
  const value = (name) => {
    const at = argv.indexOf(name);
    const raw = at < 0 ? undefined : argv[at + 1];
    if (!raw || raw.startsWith('--')) fail(`Es fehlt ${name} <wert>.`);
    return raw;
  };
  return { version: value('--version'), target: value('--target'), tag: value('--tag'), dryRun: argv.includes('--dry-run') };
}

// Wie expected-assets.mjs: der Updater ist an, wenn eine der beiden Tauri-Konfigs ihn einschaltet
function updater() {
  const conf = (name) => {
    const path = join(ROOT, 'apps/desktop/src-tauri', name);
    return existsSync(path) ? JSON.parse(readFileSync(path, 'utf8')) : {};
  };
  return updaterEnabled(conf('tauri.conf.json'), conf('tauri.release.conf.json'));
}

const { version, target, tag, dryRun } = options(process.argv.slice(2));
const files = releaseFiles(version, target, { updater: updater() });

const missing = files.filter((file) => !existsSync(join(ROOT, file.path)));
if (missing.length > 0) fail(`Es fehlen ${missing.length} Datei(en):\n${missing.map((file) => file.path).join('\n')}`);

if (dryRun) {
  console.log(`Probelauf: ${files.length} Dateien wären an ${tag} gegangen:\n${files.map((file) => file.name).join('\n')}`);
  process.exit(0);
}

// gh nimmt den Dateinamen als Namen am Release; deshalb werden die Dateien vorher passend benannt kopiert
const dir = mkdtempSync(join(tmpdir(), 'nexus-upload-'));
try {
  const paths = files.map(({ path, name }) => {
    const copy = join(dir, name);
    copyFileSync(join(ROOT, path), copy);
    return copy;
  });
  const result = spawnSync('gh', ['release', 'upload', tag, ...paths, '--clobber'], { stdio: 'inherit' });
  if (result.status !== 0) fail(`gh release upload ist gescheitert (Code ${result.status ?? result.error?.message}).`);
  console.log(`${files.length} Dateien an ${tag} gehängt.`);
} finally {
  rmSync(dir, { recursive: true, force: true });
}
