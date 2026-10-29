// Legt den gebauten Rust-Server dorthin, wo Tauri ihn erwartet:
// src-tauri/binaries/nexus-<rust-ziel>[.exe].
// Das Ziel kommt von der Tauri-CLI (TAURI_ENV_TARGET_TRIPLE), sonst aus `rustc -vV`, sonst aus der Rechnerart.
// Gebaut wird mit `cargo build -p nexus-cli --bin nexus`; die Oberfläche muss vorher gebaut sein.
// Die Ressourcenordner (Oberfläche, Bilder der Systemmeldung) bindet tauri.conf.json direkt ein;
// hier wird nur geprüft, dass die Oberfläche da ist.
// Aufruf: node apps/desktop/scripts/copy-server.mjs
import { spawnSync } from 'node:child_process';
import { chmodSync, copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { hostTarget } from '../../../scripts/targets.mjs';

const HIER = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HIER, '../../..');
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
  return hostTriple();
}

// Das Ziel des bauenden Rechners, wie rustc es nennt
function hostTriple() {
  const rustc = spawnSync('rustc', ['-vV'], { encoding: 'utf8', windowsHide: true });
  return rustc.stdout?.match(/^host: (\S+)$/m)?.[1] ?? hostTarget();
}

const ziel = target();
if (!ziel) fail(`Für ${process.platform}-${process.arch} ist kein Rust-Ziel bekannt.`);

const web = join(ROOT, 'packages/web/dist/index.html');
if (!existsSync(web)) fail(`Fehlt: ${web}. Bitte zuerst "npm run build" ausführen.`);

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
mkdirSync(BINARIES, { recursive: true });
const ablage = join(BINARIES, `nexus-${ziel}${windows ? '.exe' : ''}`);
copyFileSync(gebaut, ablage);
chmodSync(ablage, 0o755);
console.log(`Rust-Server bereitgestellt: ${ablage}`);
