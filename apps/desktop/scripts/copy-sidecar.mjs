// Legt die beiden gebündelten Programme dorthin, wo Tauri sie erwartet:
// src-tauri/binaries/harness-sidecar-<rust-ziel>[.exe] (Node-Server) und nexus-<rust-ziel>[.exe] (Rust-Server).
// Das Ziel kommt von der Tauri-CLI (TAURI_ENV_TARGET_TRIPLE), sonst aus `rustc -vV`, sonst aus der Rechnerart.
// Der Rust-Server wird hier gebaut (`cargo build -p nexus-cli --bin nexus`), der Node-Server kommt aus
// dist/sidecar (`npm run build:sidecar`).
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
console.log(`Node-Server bereitgestellt: ${ablage}`);

// Der Rust-Server: er nimmt in der App den Port an und hält den Node-Server dahinter.
// Debug oder Release richtet sich nach dem Lauf der Tauri-CLI (`tauri dev` setzt TAURI_ENV_DEBUG).
const debug = process.env.TAURI_ENV_DEBUG === 'true';
const profil = debug ? 'debug' : 'release';
const args = ['build', '-p', 'nexus-cli', '--bin', 'nexus'];
if (!debug) args.push('--release');
// Nur mit --target bauen, wenn ein anderes als das eigene Ziel gemeint ist: sonst landet das
// Ergebnis in target/<ziel>/ statt in target/<profil>/, und ein Cross-Build ohne Toolchain scheitert.
const fremdesZiel = process.env.TAURI_ENV_TARGET_TRIPLE && process.env.TAURI_ENV_TARGET_TRIPLE !== hostTriple();
if (fremdesZiel) args.push('--target', ziel);
const bau = spawnSync('cargo', args, { cwd: ROOT, stdio: 'inherit', windowsHide: true });
if (bau.error) fail(`cargo ließ sich nicht starten: ${bau.error.message}`);
if (bau.status !== 0) fail(`cargo ${args.join(' ')} endete mit Code ${bau.status}.`);

const windows = ziel.includes('windows');
const gebaut = join(ROOT, 'target', ...(fremdesZiel ? [ziel] : []), profil, windows ? 'nexus.exe' : 'nexus');
if (!existsSync(gebaut)) fail(`Fehlt: ${gebaut}.`);
const rustAblage = join(BINARIES, `nexus-${ziel}${windows ? '.exe' : ''}`);
copyFileSync(gebaut, rustAblage);
chmodSync(rustAblage, 0o755);
console.log(`Rust-Server bereitgestellt: ${rustAblage}`);

// Das Ziel des bauenden Rechners, wie rustc es nennt
function hostTriple() {
  const rustc = spawnSync('rustc', ['-vV'], { encoding: 'utf8', windowsHide: true });
  return rustc.stdout?.match(/^host: (\S+)$/m)?.[1] ?? hostTarget();
}
