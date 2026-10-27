// Prüft das gebaute Sidecar mit dem echten Programm: Server starten und auf Health, Oberfläche und server.json
// prüfen, dann CLI-Modus und Worker-Modus. Jede Prüfung gibt eine Zeile aus; ein Fehler endet mit Code 1.
// Diesen Lauf ruft später die CI auf.
// Aufruf: node scripts/sidecar/smoke-sidecar.mjs [--target <rust-ziel>] [--out <ordner>]
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { binaryName, hostTarget } from './targets.mjs';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const START_MS = 20000;
const STOP_MS = 10000;

const problems = [];

// Merkt sich das Ergebnis einer Prüfung und gibt es aus
function check(ok, name, detail = '') {
  console.log(`${ok ? 'ok  ' : 'FEHL'} ${name}${detail ? ` – ${detail}` : ''}`);
  if (!ok) problems.push(name);
}

// Liest --target und --out aus der Befehlszeile
function options(argv) {
  const value = (name) => {
    const at = argv.indexOf(name);
    return at >= 0 ? argv[at + 1] : undefined;
  };
  const target = value('--target') ?? hostTarget();
  if (!target) {
    console.error(`Für ${process.platform}-${process.arch} ist kein Rust-Ziel bekannt. Bitte --target angeben.`);
    process.exit(1);
  }
  return { target, out: resolve(value('--out') ?? join(ROOT, 'dist/sidecar')) };
}

// Sucht einen freien Port
function freePort() {
  return new Promise((done, failed) => {
    const server = createServer();
    server.on('error', failed);
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address();
      server.close(() => done(port));
    });
  });
}

const sleep = (ms) => new Promise((done) => setTimeout(done, ms));

// Beendet den Server; Windows kennt kein SIGTERM für fremde Prozessbäume
function stop(child) {
  if (process.platform === 'win32') spawnSync('taskkill', ['/pid', String(child.pid), '/T', '/F'], { stdio: 'ignore' });
  else child.kill('SIGTERM');
}

// Startet `serve`, wartet auf die Antwort des Servers und übergibt Port und Prozess an `body`; beendet danach immer
async function withServer({ binary, env, label }, body) {
  const port = await freePort();
  const child = spawn(binary, ['serve', '--port', String(port)], { env, stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true });
  let output = '';
  child.stdout.on('data', (chunk) => { output += chunk; });
  child.stderr.on('data', (chunk) => { output += chunk; });
  let exited = null;
  child.on('exit', (code) => { exited = code ?? -1; });
  try {
    const deadline = Date.now() + START_MS;
    let health = null;
    while (Date.now() < deadline && exited === null && health === null) {
      health = await ask(`http://127.0.0.1:${port}/api/health`);
      if (health === null) await sleep(150);
    }
    if (health === null) {
      check(false, `${label}: Server antwortet`, exited === null ? `kein /api/health in ${START_MS} ms` : `Prozess endete mit Code ${exited}: ${output.trim()}`);
      return;
    }
    await body({ port, health, child });
  } finally {
    if (exited === null) {
      stop(child);
      const deadline = Date.now() + STOP_MS;
      while (Date.now() < deadline && exited === null) await sleep(100);
      check(exited !== null, `${label}: Server endet auf das Beenden-Signal`);
      if (exited === null) child.kill('SIGKILL');
    }
  }
}

// Holt eine JSON-Antwort; null, wenn der Server nicht erreichbar ist oder nichts Lesbares liefert
async function ask(url) {
  try {
    const res = await fetch(url, { signal: AbortSignal.timeout(2000) });
    return await res.json();
  } catch {
    return null;
  }
}

// Holt den Text einer Antwort; null bei Fehler
async function text(url) {
  try {
    const res = await fetch(url, { signal: AbortSignal.timeout(2000) });
    return res.ok ? await res.text() : null;
  } catch {
    return null;
  }
}

// Ruft das Binary ohne „serve“ auf und liefert Rückgabecode und Ausgabe
function cli(binary, args, env) {
  return spawnSync(binary, args, { env, encoding: 'utf8', windowsHide: true, timeout: 60000 });
}

const { target, out } = options(process.argv.slice(2));
const binary = join(out, binaryName(target));
if (!existsSync(binary)) {
  console.error(`Fehlt: ${binary}. Bitte zuerst "node scripts/sidecar/build-sidecar.mjs" ausführen.`);
  process.exit(1);
}

const home = mkdtempSync(join(tmpdir(), 'harness-sidecar-smoke-'));
// Eigener HARNESS_HOME, damit der Lauf nichts am Ordner des Benutzers ändert; HARNESS_PORT bleibt außen vor
const base = { ...process.env, HARNESS_HOME: home };
delete base.HARNESS_PORT;
delete base.HARNESS_WEB_DIR;
delete base.HARNESS_TOAST_DIR;
delete base.HARNESS_SERVER_ENTRY;

try {
  // Ohne HARNESS_WEB_DIR: das Programm muss die Oberfläche von allein neben sich finden
  await withServer({ binary, env: base, label: 'Oberfläche neben dem Programm' }, async ({ port, health }) => {
    check(health?.name === 'ai-harness', 'GET /api/health meldet name "ai-harness"', JSON.stringify(health));
    check(typeof health?.version === 'string' && health.version !== '0.0.0', 'GET /api/health meldet die echte Version', String(health?.version));
    const index = await text(`http://127.0.0.1:${port}/`);
    check(typeof index === 'string' && /<!doctype html/i.test(index), 'GET / liefert index.html');
    const info = join(home, 'server.json');
    const written = existsSync(info) ? JSON.parse(readFileSync(info, 'utf8')) : null;
    check(written?.port === port, 'server.json ist geschrieben', JSON.stringify(written));
  });

  // Mit HARNESS_WEB_DIR und HARNESS_TOAST_DIR auf die Ressourcen – so bindet die Desktop-App sie ein
  const wired = { ...base, HARNESS_WEB_DIR: join(out, 'web'), HARNESS_TOAST_DIR: join(out, 'toast'), HARNESS_SQL_DIR: join(out, 'sql') };
  await withServer({ binary, env: wired, label: 'Ressourcen über Umgebungsvariablen' }, async ({ port, health }) => {
    check(health?.name === 'ai-harness', 'GET /api/health meldet name "ai-harness" (mit HARNESS_WEB_DIR)');
    const index = await text(`http://127.0.0.1:${port}/`);
    check(typeof index === 'string' && /<!doctype html/i.test(index), 'GET / liefert index.html (mit HARNESS_WEB_DIR)');
  });

  // CLI-Modus: ohne Board-Datenbank ist „board-config-missing“ mit Code 1 die erwartete Antwort, kein Absturz
  const list = cli(binary, ['board', 'list', '--json'], base);
  const answer = (() => { try { return JSON.parse(list.stdout); } catch { return null; } })();
  check([0, 1].includes(list.status), 'board list --json endet mit 0 oder dem erwarteten Fehlercode', `Code ${list.status}`);
  check(answer !== null, 'board list --json antwortet in JSON', (list.stdout || list.stderr || '').trim().slice(0, 200));

  // Unbekannte Option: eine Meldung und Code 1, kein Absturz
  const unknown = cli(binary, ['--version'], base);
  check(unknown.status === 1 && /\S/.test(unknown.stdout + unknown.stderr), '--version endet geordnet mit einer Meldung', `Code ${unknown.status}`);

  // Worker-Modus mit unbekannter Lauf-ID: geordnetes Ende mit Meldung
  const worker = cli(binary, ['--dispatch-worker', 'ARC-9999-smoke'], base);
  check(worker.status === 1 && /ARC-9999-smoke/.test(worker.stdout + worker.stderr), '--dispatch-worker mit unbekannter Lauf-ID endet geordnet', `Code ${worker.status}`);
} finally {
  rmSync(home, { recursive: true, force: true });
}

if (problems.length) {
  console.error(`\n${problems.length} Prüfung(en) fehlgeschlagen: ${problems.join(', ')}`);
  process.exit(1);
}
console.log(`\nAlle Prüfungen bestanden: ${binary}`);
