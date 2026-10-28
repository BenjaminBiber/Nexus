// Legt das gebaute Sidecar dorthin, wo Tauri es erwartet: src-tauri/binaries/harness-sidecar-<rust-ziel>[.exe].
// Das Ziel kommt von der Tauri-CLI (TAURI_ENV_TARGET_TRIPLE), sonst aus `rustc -vV`, sonst aus der Rechnerart.
// Die drei Ressourcenordner bindet tauri.conf.json direkt aus dist/sidecar/ ein; hier werden sie nur geprüft.
// Aufruf: node apps/desktop/scripts/copy-sidecar.mjs
import { spawnSync } from 'node:child_process';
import { chmodSync, copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { binaryName, hostTarget } from '../../../scripts/sidecar/targets.mjs';

const HIER = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HIER, '../../..');
const SIDECAR = join(ROOT, 'dist/sidecar');
const BINARIES = resolve(HIER, '../src-tauri/binaries');

// Bricht mit Meldung ab
function fail(message) {
  console.error(message);
  process.exit(1);
}

// Rust-Ziel: von der Tauri-CLI gesetzt, sonst der Host von rustc, sonst die Rechnerart von Node
function target() {
  const fromCli = process.env.TAURI_ENV_TARGET_TRIPLE;
  if (fromCli) return fromCli;
  const rustc = spawnSync('rustc', ['-vV'], { encoding: 'utf8', windowsHide: true });
  const host = rustc.stdout?.match(/^host: (\S+)$/m)?.[1];
  return host ?? hostTarget();
}

const ziel = target();
if (!ziel) fail(`Für ${process.platform}-${process.arch} ist kein Rust-Ziel bekannt.`);

const name = binaryName(ziel);
const quelle = join(SIDECAR, name);
if (!existsSync(quelle)) {
  fail(`Fehlt: ${quelle}. Bitte zuerst "npm run build:sidecar" ausführen (Ziel ${ziel}).`);
}
for (const ordner of ['web', 'toast', 'sql']) {
  if (!existsSync(join(SIDECAR, ordner))) fail(`Fehlt: ${join(SIDECAR, ordner)}. Bitte zuerst "npm run build:sidecar" ausführen.`);
}

mkdirSync(BINARIES, { recursive: true });
const ablage = join(BINARIES, name);
copyFileSync(quelle, ablage);
chmodSync(ablage, 0o755);
console.log(`Sidecar bereitgestellt: ${ablage}`);
