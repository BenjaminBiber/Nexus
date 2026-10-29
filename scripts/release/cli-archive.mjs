// Packt die CLI für den Betrieb ohne App: das Rust-Programm `nexus` und das gebündelte Plugin
// für Claude Code. Ergebnis: dist/release/nexus-cli-<version>-<ziel>.tar.gz (Windows: .zip).
// Der Pfad des Archivs geht auf stdout, alles andere auf stderr – so lässt er sich im
// Release-Workflow direkt in eine Variable übernehmen.
// Voraussetzung: `npm run build` und `cargo build --release -p nexus-cli --bin nexus --target <ziel>`.
// Aufruf: node scripts/release/cli-archive.mjs --version <X.Y.Z> [--target <rust-ziel>] [--out <ordner>] [--root <dir>]
import { spawnSync } from 'node:child_process';
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { hostTarget } from '../targets.mjs';
import { archiveName, archiveStem, binaryName, isWindowsTarget } from './lib/CliArchive.mjs';

const HIER = dirname(fileURLToPath(import.meta.url));

// Bricht mit Meldung ab
function fail(message) {
  console.error(message);
  process.exit(1);
}

// Liest --version, --target, --out und --root aus der Befehlszeile
function options(argv) {
  const value = (name) => {
    const at = argv.indexOf(name);
    if (at < 0) return undefined;
    const raw = argv[at + 1];
    if (raw === undefined || raw.startsWith('--')) fail(`Zu ${name} fehlt der Wert.`);
    return raw;
  };
  const version = value('--version');
  if (!version) fail('Es fehlt --version <X.Y.Z>.');
  const target = value('--target') ?? hostTarget();
  if (!target) fail(`Für ${process.platform}-${process.arch} ist kein Rust-Ziel bekannt. Bitte --target angeben.`);
  const root = resolve(value('--root') ?? resolve(HIER, '../..'));
  return { version, target, root, out: resolve(value('--out') ?? join(root, 'dist/release')) };
}

// Führt einen Befehl aus und bricht bei Fehler ab; seine Ausgabe geht nach stderr, damit stdout der Pfad bleibt
function run(command, args, options, hint) {
  const result = spawnSync(command, args, { ...options, stdio: ['ignore', 'pipe', 'pipe'], encoding: 'utf8', windowsHide: true });
  if (result.stdout) process.stderr.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
  if (result.error) fail(`${command} ließ sich nicht starten: ${result.error.message}`);
  if (result.status !== 0) fail(`${hint} (Code ${result.status})`);
}

// Kurze Anleitung im Archiv; sie nennt nur, was ohne App nötig ist
function readme(version, target) {
  return [
    `# Nexus CLI ${version} (${target})`,
    '',
    'Enthält den Befehl `harness` als eigenständiges Programm und das Plugin für Claude Code.',
    '',
    '- `bin/` – das Programm. In den Pfad legen oder von hier aufrufen: `bin/harness board list`.',
    '- `plugin/` – das Plugin. In Claude Code über diesen Ordner einbinden.',
    '',
    'Das Programm bringt alles mit; Node muss nicht installiert sein. Auf macOS ist es unsigniert:',
    'beim ersten Start meldet sich Gatekeeper, der Start lässt sich in den Systemeinstellungen freigeben.',
    '',
  ].join('\n');
}

const { version, target, root, out } = options(process.argv.slice(2));

// Das Rust-Programm ist zugleich die CLI und der Server (`nexus serve`)
const quelle = join(root, 'target', target, 'release', isWindowsTarget(target) ? 'nexus.exe' : 'nexus');
if (!existsSync(quelle)) {
  fail(`Fehlt: ${quelle}. Bitte zuerst "cargo build --release -p nexus-cli --bin nexus --target ${target}" ausführen.`);
}

const stem = archiveStem(version, target);
const stage = mkdtempSync(join(tmpdir(), 'nexus-cli-'));
const inhalt = join(stage, stem);

try {
  mkdirSync(join(inhalt, 'bin'), { recursive: true });
  const programm = join(inhalt, 'bin', binaryName(target));
  copyFileSync(quelle, programm);
  chmodSync(programm, 0o755);
  writeFileSync(join(inhalt, 'LIESMICH.md'), readme(version, target));

  // Das Plugin baut sein eigenes Skript; HARNESS_PLUGIN_OUT lenkt es in das Archiv statt in packages/plugin/dist
  run(process.execPath, [join(root, 'packages/plugin/scripts/build-plugin.mjs')], {
    cwd: root,
    env: { ...process.env, HARNESS_PLUGIN_OUT: join(inhalt, 'plugin') },
  }, 'build-plugin.mjs ist fehlgeschlagen');

  mkdirSync(out, { recursive: true });
  const archiv = join(out, archiveName(version, target));
  rmSync(archiv, { force: true });
  // tar liegt auf allen drei Plattformen bei; unter Windows ist es bsdtar und schreibt mit -a ein Zip
  const args = isWindowsTarget(target) ? ['-a', '-cf', archiv] : ['-czf', archiv];
  run('tar', [...args, '-C', stage, stem], {}, 'tar konnte das Archiv nicht schreiben');

  console.error(`CLI-Archiv gebaut: ${archiv}`);
  console.log(archiv);
} finally {
  rmSync(stage, { recursive: true, force: true });
}
