// Listet die erwarteten Dateinamen eines Releases oder prüft eine vorhandene Liste dagegen.
// Liest `apps/desktop/src-tauri/tauri.conf.json`, um zu erkennen, ob der Updater aktiv ist
// (`bundle.createUpdaterArtifacts`, P6-7) – nur dann gehören `latest.json` und die `.sig`-Dateien dazu.
// Aufruf: node scripts/release/expected-assets.mjs <version> [--check <datei mit Namen je Zeile>]
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { assetNames, missingAssets, updaterEnabled } from './lib/ExpectedAssets.mjs';

const HIER = dirname(fileURLToPath(import.meta.url));
const TAURI_CONF = resolve(HIER, '../../apps/desktop/src-tauri/tauri.conf.json');

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

const { version, check } = options(process.argv.slice(2));
const tauriConf = JSON.parse(readFileSync(TAURI_CONF, 'utf8'));
const expected = assetNames(version, { updater: updaterEnabled(tauriConf) });

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
