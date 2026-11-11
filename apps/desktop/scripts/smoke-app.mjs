// Prüft die gebaute Debug-App im echten Lauf: Server finden oder im eigenen Prozess starten, Oberfläche aus
// den Dateien der App laden und beim Beenden nur den selbst gestarteten Server mitnehmen. Jede Prüfung gibt eine Zeile aus; ein Fehler endet mit Code 1.
// Der Lauf öffnet dreimal kurz ein Fenster und arbeitet in einem eigenen HARNESS_HOME auf einem freien Port.
// Unter Windows gibt es kein geordnetes Beenden von außen; dort prüft ein Mensch das Schließen über das Fenster.
// Voraussetzung: npm run tauri -w apps/desktop -- build --debug --no-bundle
// Aufruf: node apps/desktop/scripts/smoke-app.mjs
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { createServer as createHttpServer } from 'node:http';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
const APP_NAME = process.platform === 'win32' ? 'nexus-desktop.exe' : 'nexus-desktop';
// Die App liegt im gemeinsamen target/ des Cargo-Workspace; wer src-tauri einzeln baut, findet sie daneben
const APP_ORTE = [join(ROOT, 'target/debug', APP_NAME), join(ROOT, 'apps/desktop/src-tauri/target/debug', APP_NAME)];
const APP = APP_ORTE.find((ort) => existsSync(ort)) ?? APP_ORTE[0];
// Der Rust-Server; der zweite Lauf startet ihn selbst, damit auf dem Port schon einer läuft
const SERVER = join(ROOT, 'target/debug', process.platform === 'win32' ? 'nexus.exe' : 'nexus');
// Die App öffnet ein Fenster und startet den Server; beim ersten Lauf dauert das
const START_MS = 60000;
const STOP_MS = 20000;
// Protokollzeile, sobald das Fenster die Oberfläche aus den Dateien der App lädt (NXS-0100)
const WINDOW_LOADED = /Das Fenster lädt .+ aus den Dateien der App/;
// Nur dort lässt sich die App von außen geordnet beenden
const GRACEFUL = process.platform !== 'win32';

const problems = [];

// Merkt sich das Ergebnis einer Prüfung und gibt es als eine Zeile aus
function check(ok, name, detail = '') {
  const kurz = detail.replace(/\s+/g, ' ').trim();
  console.log(`${ok ? 'ok  ' : 'FEHL'} ${name}${kurz ? ` – ${kurz}` : ''}`);
  if (!ok) problems.push(name);
}

const sleep = (ms) => new Promise((done) => setTimeout(done, ms));

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

// Holt die Health-Antwort; null, wenn dort niemand antwortet
async function health(port) {
  try {
    const res = await fetch(`http://127.0.0.1:${port}/api/health`, { signal: AbortSignal.timeout(2000) });
    return await res.json();
  } catch {
    return null;
  }
}

// Prozessangaben aus server.json des Testordners; null, wenn es sie nicht gibt
function info(home) {
  const file = join(home, 'server.json');
  if (!existsSync(file)) return null;
  try {
    return JSON.parse(readFileSync(file, 'utf8'));
  } catch {
    return null;
  }
}

// Wartet auf server.json: Der Server öffnet den Port, bevor er sie schreibt, darum reicht die Health-Antwort
// als Zeitpunkt nicht aus. null, wenn sie nicht kommt.
async function waitForInfo(home, ms = 5000) {
  await waitFor(() => info(home) !== null, ms);
  return info(home);
}

// Läuft der Prozess noch?
function alive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return error.code === 'EPERM';
  }
}

// Beendet einen Prozess; Windows kennt kein SIGTERM
function stop(child) {
  if (process.platform === 'win32') spawnSync('taskkill', ['/pid', String(child.pid), '/F'], { stdio: 'ignore' });
  else child.kill('SIGTERM');
}

// Wartet, bis die Bedingung zutrifft; false, wenn die Frist abläuft
async function waitFor(condition, ms) {
  const deadline = Date.now() + ms;
  while (Date.now() < deadline) {
    if (await condition()) return true;
    await sleep(200);
  }
  return condition();
}

// Startet ein Programm und sammelt seine Ausgabe; `exited` bleibt null, solange es läuft
function start(command, args, env) {
  const child = spawn(command, args, { env, stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true });
  const run = { child, output: '', exited: null };
  child.stdout.on('data', (chunk) => { run.output += chunk; });
  child.stderr.on('data', (chunk) => { run.output += chunk; });
  child.on('exit', (code, signal) => { run.exited = code ?? `Signal ${signal}`; });
  return run;
}

// Umgebung der App: eigener Ordner, eigener Port, keine Einstellungen des Entwicklers
function appEnv(home, port) {
  const env = { ...process.env, HARNESS_HOME: home, HARNESS_PORT: String(port) };
  for (const name of ['HARNESS_WEB_DIR', 'HARNESS_TOAST_DIR']) delete env[name];
  return env;
}

// Wartet auf die Health-Antwort; bricht ab, wenn das Programm vorher endet
async function waitForHealth(port, run) {
  const deadline = Date.now() + START_MS;
  while (Date.now() < deadline && run.exited === null) {
    const answer = await health(port);
    if (answer !== null) return answer;
    await sleep(200);
  }
  return null;
}

// Erster Lauf: Der Port ist frei, die App startet den Server und nimmt ihn beim Beenden mit
async function appStartetDenServer() {
  const label = 'App startet den Server';
  const home = mkdtempSync(join(tmpdir(), 'nexus-smoke-eigen-'));
  const port = await freePort();
  const app = start(APP, [], appEnv(home, port));
  let server = null;
  try {
    const answer = await waitForHealth(port, app);
    check(answer?.name === 'ai-harness', `${label}: GET /api/health meldet name "ai-harness"`, answer === null ? `keine Antwort in ${START_MS} ms, App ${app.exited === null ? 'läuft' : `endete mit ${app.exited}`}: ${app.output.trim().slice(-300)}` : JSON.stringify(answer));
    if (answer === null) return;

    const written = await waitForInfo(home);
    server = written?.pid;
    check(written?.port === port, `${label}: server.json nennt den Port der App`, JSON.stringify(written));
    // Der Server läuft im Prozess der App (NXS-0099)
    check(server === app.child.pid, `${label}: server.json nennt die App, in deren Prozess der Server läuft`, `pid ${server}, App ${app.child.pid}`);
    check(await waitFor(() => WINDOW_LOADED.test(app.output), 10000), `${label}: das Fenster lädt die Oberfläche aus den Dateien der App`);

    if (!GRACEFUL) {
      console.log('    (das geordnete Beenden prüft unter Windows ein Mensch über das Fenster)');
      return;
    }
    stop(app.child);
    check(await waitFor(() => app.exited !== null, STOP_MS), `${label}: die App endet auf SIGTERM`, String(app.exited));
    check(await waitFor(() => !alive(server), 5000), `${label}: der von der App gestartete Server ist mit beendet`, `pid ${server}`);
    check(await waitFor(async () => (await health(port)) === null, 5000), `${label}: auf dem Port antwortet niemand mehr`);
    check(await waitFor(() => info(home) === null, 5000), `${label}: server.json ist entfernt`, JSON.stringify(info(home)));
  } finally {
    if (app.exited === null) app.child.kill('SIGKILL');
    if (server && alive(server)) process.kill(server, 'SIGKILL');
    rmSync(home, { recursive: true, force: true });
  }
}

// Zweiter Lauf: Auf dem Port läuft schon ein Server, die App verbindet sich und lässt ihn in Ruhe
async function serverLaeuftSchon() {
  const label = 'Server läuft schon';
  const home = mkdtempSync(join(tmpdir(), 'nexus-smoke-fremd-'));
  const port = await freePort();
  const env = appEnv(home, port);
  const server = start(SERVER, ['serve', '--port', String(port)], env);
  let app = null;
  try {
    const answer = await waitForHealth(port, server);
    check(answer?.name === 'ai-harness', `${label}: der vorher gestartete Server antwortet`, answer === null ? server.output.trim().slice(-300) : JSON.stringify(answer));
    if (answer === null) return;
    const before = await waitForInfo(home);
    check(before?.pid === server.child.pid, `${label}: server.json nennt den vorher gestarteten Server`, JSON.stringify(before));

    app = start(APP, [], env);
    const connected = await waitFor(() => WINDOW_LOADED.test(app.output), START_MS);
    check(connected, `${label}: die App lädt die Oberfläche`, connected ? '' : app.output.trim().slice(-300));
    const alleine = /läuft schon ein Nexus-Server/.test(app.output);
    check(alleine, `${label}: die App startet keinen zweiten Server`, alleine ? '' : app.output.trim().slice(-300));
    check(info(home)?.pid === before?.pid, `${label}: server.json nennt weiter denselben Server`, JSON.stringify(info(home)));

    if (!GRACEFUL) {
      console.log('    (das geordnete Beenden prüft unter Windows ein Mensch über das Fenster)');
      return;
    }
    stop(app.child);
    check(await waitFor(() => app.exited !== null, STOP_MS), `${label}: die App endet auf SIGTERM`, String(app.exited));
    check(server.exited === null && alive(server.child.pid), `${label}: der schon laufende Server läuft weiter`, `pid ${server.child.pid}, Ende ${server.exited}`);
    check((await health(port))?.name === 'ai-harness', `${label}: der Server antwortet nach dem Beenden der App weiter`);
    check(info(home)?.pid === before?.pid, `${label}: server.json ist unverändert`, JSON.stringify(info(home)));
  } finally {
    if (app && app.exited === null) app.child.kill('SIGKILL');
    if (server.exited === null) {
      stop(server.child);
      await waitFor(() => server.exited !== null, STOP_MS);
      if (server.exited === null) server.child.kill('SIGKILL');
    }
    rmSync(home, { recursive: true, force: true });
  }
}

// Dritter Lauf: Auf dem Port sitzt ein fremdes Programm; die App bleibt auf der Startseite und meldet das
async function fremdesProgrammAufDemPort() {
  const label = 'Fremdes Programm auf dem Port';
  const home = mkdtempSync(join(tmpdir(), 'nexus-smoke-besetzt-'));
  const fremd = createHttpServer((_request, response) => {
    response.writeHead(200, { 'content-type': 'application/json' });
    response.end('{"name":"etwas-anderes"}');
  });
  await new Promise((done) => fremd.listen(0, '127.0.0.1', done));
  const port = fremd.address().port;
  const app = start(APP, [], appEnv(home, port));
  try {
    const gemeldet = await waitFor(() => /ist von einem anderen Programm belegt/.test(app.output), START_MS);
    check(gemeldet, `${label}: die App meldet den belegten Port`, gemeldet ? '' : app.output.trim().slice(-300));
    check(!/Das Fenster lädt/.test(app.output), `${label}: das Fenster bleibt auf der Startseite`);
    check(info(home) === null, `${label}: die App schreibt keine server.json`, JSON.stringify(info(home)));
    check(app.exited === null, `${label}: die App läuft weiter und zeigt die Meldung`, String(app.exited));
  } finally {
    if (app.exited === null) {
      stop(app.child);
      await waitFor(() => app.exited !== null, STOP_MS);
      if (app.exited === null) app.child.kill('SIGKILL');
    }
    await new Promise((done) => fremd.close(done));
    rmSync(home, { recursive: true, force: true });
  }
}

if (!existsSync(APP)) {
  console.error(`Fehlt: ${APP}. Bitte zuerst "npm run tauri -w apps/desktop -- build --debug --no-bundle" ausführen.`);
  process.exit(1);
}
if (!existsSync(SERVER)) {
  console.error(`Fehlt: ${SERVER}. Bitte zuerst "cargo build -p nexus-cli --bin nexus" ausführen.`);
  process.exit(1);
}

await appStartetDenServer();
await serverLaeuftSchon();
await fremdesProgrammAufDemPort();

if (problems.length) {
  console.error(`\n${problems.length} Prüfung(en) fehlgeschlagen: ${problems.join(', ')}`);
  process.exit(1);
}
console.log(`\nAlle Prüfungen bestanden: ${APP}`);
