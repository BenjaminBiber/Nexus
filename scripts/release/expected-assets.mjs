// Listet die erwarteten Dateinamen eines Releases oder prüft eine vorhandene Liste dagegen.
// Liest `apps/desktop/src-tauri/tauri.conf.json` und die Release-Konfig daneben
// (`tauri.release.conf.json`, die der Workflow über `--config` dazugibt), um zu erkennen, ob der
// Updater aktiv ist (`bundle.createUpdaterArtifacts`) – nur dann gehören `latest.json` und die
// `.sig`-Dateien dazu. Je Ordner unter `themes/` gehören `<id>-<version>.nexus` und `nexus-themes.json` dazu.
// Aufruf: node scripts/release/expected-assets.mjs <version> [--check <datei mit Namen je Zeile>]
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { assetNames, missingAssets, updaterEnabled } from './lib/ExpectedAssets.mjs';

const HIER = dirname(fileURLToPath(import.meta.url));
const TAURI_CONF = resolve(HIER, '../../apps/desktop/src-tauri/tauri.conf.json');
const TAURI_RELEASE_CONF = resolve(HIER, '../../apps/desktop/src-tauri/tauri.release.conf.json');
const THEMES = resolve(HIER, '../../themes');

// Bricht mit Meldung ab
function fail(message) {
  console.error(message);
  process.exit(1);
}

// Liest Version und optional --check aus der Befehlszeile
function options(argv) {
  const version = argv[0];
  if (!version || version.startsWith('--')) fail('Aufruf: expected-assets.mjs <version> [--check <datei>]');
  const at = argv.indexOf('--check');
  const check = at < 0 ? undefined : argv[at + 1];
  if (at >= 0 && !check) fail('Zu --check fehlt der Pfad der Datei.');
  return { version, check };
}

// Liest eine Tauri-Konfig; eine fehlende Datei zählt als leere Konfig
function konfig(pfad) {
  return existsSync(pfad) ? JSON.parse(readFileSync(pfad, 'utf8')) : {};
}

// Manifeste der mitgelieferten Themes (themes/<id>/manifest.json, ohne Punkt- und Unterstrich-Ordner wie build-themes.mjs)
function themeManifeste() {
  if (!existsSync(THEMES)) return [];
  return readdirSync(THEMES, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && !/^[._]/.test(entry.name))
    .map((entry) => join(THEMES, entry.name, 'manifest.json'))
    .filter((pfad) => existsSync(pfad))
    .map((pfad) => JSON.parse(readFileSync(pfad, 'utf8')));
}

const { version, check } = options(process.argv.slice(2));
const updater = updaterEnabled(konfig(TAURI_CONF), konfig(TAURI_RELEASE_CONF));
const expected = assetNames(version, { updater, themes: themeManifeste() });

if (!check) {
  console.log(expected.join('\n'));
  process.exit(0);
}

const actual = readFileSync(check, 'utf8').split('\n').map((line) => line.trim()).filter(Boolean);
const missing = missingAssets(expected, actual);
if (missing.length > 0) {
  console.error(`Am Release fehlen ${missing.length} Datei(en):\n${missing.join('\n')}`);
  process.exit(1);
}
console.log(`Alle ${expected.length} erwarteten Dateien sind am Release.`);
