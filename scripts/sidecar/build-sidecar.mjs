// Baut das Sidecar der Desktop-App: bündelt Server, Worker und CLI mit esbuild zu einer CommonJS-Datei und
// macht daraus eine Node Single Executable Application (SEA). Daneben landen die Ressourcen, die der Server
// vom Datenträger liest (Oberfläche, Benachrichtigungsbilder, SQL-Skripte).
// Voraussetzung: `npm run build` und ein eigenständiges Node (von nodejs.org). Node aus einer Paketverwaltung
// ist oft als Shared Library gebaut; dort ist SEA abgeschaltet und der Bau endet mit einem Hinweis (SEA_DISABLED).
// Ergebnis: dist/sidecar/harness-sidecar-<rust-ziel>[.exe]
// Aufruf: node scripts/sidecar/build-sidecar.mjs [--target <rust-ziel>] [--node-binary <pfad>] [--out <ordner>]
import { spawnSync } from 'node:child_process';
import { chmodSync, copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { bundleForNode } from '../nodeBundle.mjs';
import { binaryName, hostTarget } from './targets.mjs';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

// Node bettet den Blob seit 25.5 selbst ein; davor braucht es postject
const BUILD_SEA_SINCE = [25, 5, 0];
// Marke im Node-Programm, an der postject den Schalter „ich bin ein SEA“ umlegt (aus der Node-Doku)
const SENTINEL = 'NODE_SEA_FUSE_fce680ab2cc467b6e072b8b5df1996b2';
// Fest gewählte Fassung, damit der Build überall dasselbe Werkzeug verwendet
const POSTJECT = 'postject@1.0.0-alpha.6';

// Liest --target, --node-binary und --out aus der Befehlszeile
function options(argv) {
  const value = (name) => {
    const at = argv.indexOf(name);
    if (at < 0) return undefined;
    const raw = argv[at + 1];
    if (raw === undefined || raw.startsWith('--')) fail(`Zu ${name} fehlt der Wert.`);
    return raw;
  };
  const target = value('--target') ?? hostTarget();
  if (!target) fail(`Für ${process.platform}-${process.arch} ist kein Rust-Ziel bekannt. Bitte --target angeben.`);
  return {
    target,
    nodeBinary: value('--node-binary'),
    out: resolve(value('--out') ?? join(ROOT, 'dist/sidecar')),
  };
}

// Bricht mit Meldung ab
function fail(message) {
  console.error(message);
  process.exit(1);
}

// Prüft, ob alle Eingaben gebaut sind
function assertInputs() {
  for (const file of ['packages/cli/dist/main.js', 'packages/server/dist/main.js', 'packages/web/dist/index.html', 'packages/core/dist/index.js']) {
    if (!existsSync(join(ROOT, file))) fail(`Fehlt: ${file}. Bitte zuerst "npm run build" ausführen.`);
  }
}

// Führt einen Befehl aus und bricht bei Fehler ab; „sea disabled“ bekommt einen eigenen Hinweis
function run(command, args, hint) {
  const result = spawnSync(command, args, { stdio: ['ignore', 'inherit', 'pipe'], encoding: 'utf8', windowsHide: true });
  const stderr = result.stderr ?? '';
  if (stderr) process.stderr.write(stderr);
  if (result.error) fail(`${command} ließ sich nicht starten: ${result.error.message}`);
  if (result.status === 0) return;
  if (/Single executable application is disabled/.test(stderr)) fail(SEA_DISABLED);
  fail(hint ? `${hint} (Code ${result.status})` : `${command} endete mit Code ${result.status}.`);
}

const SEA_DISABLED = [
  'Dieses Node kann keine Single Executable Application bauen: es ist als Shared Library gebaut (etwa das Node aus Homebrew).',
  'Den Build mit einem eigenständigen Node von nodejs.org ausführen, zum Beispiel:',
  '  /pfad/zu/node scripts/sidecar/build-sidecar.mjs',
  'Prüfen lässt sich das mit: node -p "process.config.variables.node_shared"  (muss false sein)',
].join('\n');

// true, wenn das laufende Node den Blob selbst einbetten kann (--build-sea)
function hasBuildSea() {
  const [major, minor, patch] = process.versions.node.split('.').map(Number);
  const [wantMajor, wantMinor, wantPatch] = BUILD_SEA_SINCE;
  if (major !== wantMajor) return major > wantMajor;
  if (minor !== wantMinor) return minor > wantMinor;
  return patch >= wantPatch;
}

// Warnt, wenn das übergebene Node nicht zum bauenden passt: der Blob gilt nur für genau diese Fassung
function checkNodeBinary(path) {
  if (!existsSync(path)) fail(`Das Node-Programm ${path} gibt es nicht.`);
  const result = spawnSync(path, ['--version'], { encoding: 'utf8', windowsHide: true });
  if (result.status !== 0) {
    console.warn(`Warnung: ${path} ließ sich nicht ausführen, die Fassung bleibt ungeprüft. Sie muss ${process.version} sein.`);
    return;
  }
  const version = result.stdout.trim();
  if (version !== process.version) {
    fail(`Das Node-Programm ${path} hat die Fassung ${version}, das bauende Node ${process.version}. Der Blob passt nur zu genau derselben Fassung.`);
  }
}

// Bettet den Blob mit postject in eine Kopie des Node-Programms ein (Weg für Node vor 25.5 und für --node-binary)
function injectWithPostject({ host, blob, binary, target }) {
  const macho = target.includes('apple-darwin');
  copyFileSync(host, binary);
  chmodSync(binary, 0o755);
  // Eine vorhandene Signatur gilt nach dem Einbetten nicht mehr; macOS verweigert den Start dann
  if (macho && process.platform === 'darwin') spawnSync('codesign', ['--remove-signature', binary], { stdio: 'ignore' });
  const npx = process.platform === 'win32' ? 'npx.cmd' : 'npx';
  run(npx, [
    '--yes', POSTJECT, binary, 'NODE_SEA_BLOB', blob,
    '--sentinel-fuse', SENTINEL,
    ...(macho ? ['--macho-segment-name', 'NODE_SEA'] : []),
  ], `postject (${POSTJECT}) konnte den Blob nicht einbetten`);
  // Ad-hoc-Signatur, damit macOS das Programm startet; die echte Signatur kommt später im Release-Build
  if (macho && process.platform === 'darwin') run('codesign', ['--sign', '-', '--force', binary], 'codesign ist fehlgeschlagen');
}

const { target, nodeBinary, out } = options(process.argv.slice(2));
assertInputs();
// Ohne eigenes Node-Programm entsteht ein Programm für den bauenden Rechner; unter einem fremden Zielnamen
// wäre es eine Falle. Für ein anderes Ziel gehört das Node-Programm dieses Ziels dazu.
if (!nodeBinary && target !== hostTarget()) {
  fail(`Für das Ziel ${target} braucht es das Node-Programm dieses Ziels: --node-binary <pfad> angeben (Fassung ${process.version}).`);
}

const buildDir = join(out, 'build');
const bundle = join(buildDir, 'sidecar.cjs');
const configFile = join(buildDir, 'sea-config.json');
const binary = join(out, binaryName(target));

rmSync(out, { recursive: true, force: true });
mkdirSync(buildDir, { recursive: true });

// Die Version steht im Bündel als Konstante: im Sidecar gibt es keine package.json, die der Server lesen könnte
const version = JSON.parse(readFileSync(join(ROOT, 'packages/server/package.json'), 'utf8')).version;
await bundleForNode({
  entry: join(ROOT, 'scripts/sidecar/entry.mjs'),
  outfile: bundle,
  format: 'cjs',
  define: { 'process.env.HARNESS_BUNDLED_VERSION': JSON.stringify(version) },
});

// Ohne Snapshot und ohne Code-Cache ist der Blob unabhängig von der Rechnerart – nur so geht der Bau für ein anderes Ziel
const useBuildSea = hasBuildSea() && !nodeBinary;
const blob = join(buildDir, 'sidecar.blob');
writeFileSync(configFile, JSON.stringify({
  main: bundle,
  output: useBuildSea ? binary : blob,
  disableExperimentalSEAWarning: true,
  useSnapshot: false,
  useCodeCache: false,
}, null, 2) + '\n');

if (useBuildSea) {
  run(process.execPath, ['--build-sea', configFile], 'node --build-sea ist fehlgeschlagen');
  chmodSync(binary, 0o755);
  if (target.includes('apple-darwin') && process.platform === 'darwin') {
    run('codesign', ['--sign', '-', '--force', binary], 'codesign ist fehlgeschlagen');
  }
} else {
  if (nodeBinary) checkNodeBinary(nodeBinary);
  run(process.execPath, ['--experimental-sea-config', configFile], 'node --experimental-sea-config ist fehlgeschlagen');
  injectWithPostject({ host: nodeBinary ?? process.execPath, blob, binary, target });
}

// Diese drei Ordner liest der Server vom Datenträger; die Desktop-App bindet sie als Ressourcen neben dem Programm ein
cpSync(join(ROOT, 'packages/web/dist'), join(out, 'web'), { recursive: true });
cpSync(join(ROOT, 'packages/server/assets/toast'), join(out, 'toast'), { recursive: true });
cpSync(join(ROOT, 'packages/db/sql'), join(out, 'sql'), { recursive: true });

console.log(`Sidecar gebaut: ${binary}`);
console.log(`Ressourcen: ${join(out, 'web')}, ${join(out, 'toast')}, ${join(out, 'sql')}`);
