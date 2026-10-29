import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { arch, cpus, platform, release, tmpdir, totalmem } from 'node:os';
import { dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { DEFAULT_TOLERANCE_PCT, compareMetrics, formatComparison, machineWarning, worseRows } from './lib/BaselineCompare.mjs';
import { BenchProject } from './lib/BenchProject.mjs';
import { BoardSeeder } from './lib/BoardSeeder.mjs';
import { HookBench } from './lib/HookBench.mjs';
import { ProcessMemory } from './lib/ProcessMemory.mjs';
import { ServerProcess } from './lib/ServerProcess.mjs';
import { SseBench } from './lib/SseBench.mjs';
import { mean, median, p95, round1 } from './lib/Stats.mjs';

// Version des Ausgabeformats von contract/baseline.json
const SCHEMA = 1;
// Startbefehl des Rust-Servers; `{port}` wird durch den freien Port ersetzt
const DEFAULT_SERVER_CMD = 'target/release/nexus serve --port {port}';
// Programmdatei, die der Standardbefehl braucht (nur dann geprüft)
const DEFAULT_ENTRY = join('target', 'release', process.platform === 'win32' ? 'nexus.exe' : 'nexus');
// Umfang einer vollen Messung und der schnellen Messung (--quick, für Tests und Rauchtest)
const FULL = { tickets: 500, runs: 10, loads: 100, hookRuns: 30, sseRuns: 20, dbRuns: 10 };
const QUICK = { tickets: 50, runs: 3, loads: 10, hookRuns: 5, sseRuns: 5, dbRuns: 3 };
// Anfragen vor der warmen Messreihe, damit nicht der erste Zugriff die Werte prägt
const WARMUP_LOADS = 5;
// Ruhezeit vor der Speichermessung
const IDLE_MS = 5000;
// Benutzer der Messung (board.yaml braucht eine GUID und einen Namen)
const BENCH_USER = { userId: '11111111-1111-1111-1111-111111111111', userName: 'Bench' };
// Hook-Ereignisse der Messung: Messgröße → Ereignis des Hooks
const HOOK_METRICS = { hookSessionStartMs: 'session-start', hookUserPromptMs: 'user-prompt' };
// Messgrößen, die dieses Skript schreibt; fremde Einträge in der Datei bleiben unberührt
const OWN_METRICS = [
  'serverColdStartMs', 'boardFirstLoadMs', 'boardLoadMs', 'serverRssIdleMb',
  ...Object.keys(HOOK_METRICS), 'sseSessionsLatencyMs', 'sseDbBoardLatencyMs',
];

const HELP = `Messskript für die Ausgangswerte der Migration.

  node scripts/bench/bench.mjs [--out <datei>] [--server-cmd "<befehl mit {port}>"] [--hook-cmd "<befehl>"]
                               [--tickets ${FULL.tickets}] [--runs ${FULL.runs}] [--loads ${FULL.loads}]
                               [--hook-runs ${FULL.hookRuns}] [--sse-runs ${FULL.sseRuns}] [--db-runs ${FULL.dbRuns}]
                               [--quick] [--hyperfine] [--compare <datei> [--tolerance ${DEFAULT_TOLERANCE_PCT}]]

  --out <datei>       schreibt das Ergebnis zusätzlich in diese Datei (z. B. contract/baseline.json)
  --server-cmd <text> Startbefehl des zu messenden Servers; {port} wird ersetzt
                      (Standard: ${DEFAULT_SERVER_CMD})
  --hook-cmd <text>   Befehl des Hooks ohne Ereignis (Standard: das gebaute Plugin-Bündel aus diesem Repo)
  --tickets <zahl>    Tickets, mit denen das Board befüllt wird
  --runs <zahl>       Zahl der gemessenen Kaltstarts
  --loads <zahl>      Zahl der gemessenen warmen Ladevorgänge
  --hook-runs <zahl>  Zahl der gemessenen Hook-Aufrufe je Ereignis
  --sse-runs <zahl>   Zahl der gemessenen sessions-Meldungen des Ereignisstroms
  --db-runs <zahl>    Zahl der gemessenen dbboard-Meldungen des Ereignisstroms
  --quick             kleine Messung (${QUICK.tickets} Tickets, ${QUICK.runs} Starts, ${QUICK.loads} Ladevorgänge,
                      ${QUICK.hookRuns} Hook-Aufrufe, ${QUICK.sseRuns} sessions- und ${QUICK.dbRuns} dbboard-Meldungen)
  --hyperfine         misst die Hooks mit hyperfine, falls es im PATH liegt (sonst eigene Messung)
  --compare <datei>   vergleicht das Ergebnis mit dieser Datei und endet mit Code 1, wenn ein Wert schlechter ist
  --tolerance <zahl>  Toleranz des Vergleichs in Prozent (Standard ${DEFAULT_TOLERANCE_PCT})
`;

// Wartet eine Weile
const delay = (ms) => new Promise((done) => setTimeout(done, ms));
// Fortschritt geht auf stderr, damit auf stdout nur das JSON steht
const log = (line) => process.stderr.write(`${line}\n`);

// Namen der Zahlenargumente auf ihre Felder
const NUMBER_ARGS = {
  '--tickets': 'tickets', '--runs': 'runs', '--loads': 'loads',
  '--hook-runs': 'hookRuns', '--sse-runs': 'sseRuns', '--db-runs': 'dbRuns',
};

// Liest die Argumente; ausdrücklich genannte Zahlen gelten auch zusammen mit --quick
export function parseArgs(argv) {
  const options = {
    out: null, serverCmd: DEFAULT_SERVER_CMD, hookCmd: null, help: false,
    quick: argv.includes('--quick'), hyperfine: argv.includes('--hyperfine'),
    compare: null, tolerance: DEFAULT_TOLERANCE_PCT,
  };
  const numbers = {};
  let index = 0;
  while (index < argv.length) {
    const arg = argv[index];
    const value = argv[index + 1];
    const needsValue = () => {
      if (value === undefined) throw new Error(`${arg} braucht einen Wert.`);
      index += 2;
      return value;
    };
    if (arg === '--quick' || arg === '--hyperfine') index += 1;
    else if (arg === '--help' || arg === '-h') {
      options.help = true;
      index += 1;
    } else if (arg === '--out') options.out = needsValue();
    else if (arg === '--server-cmd') options.serverCmd = needsValue();
    else if (arg === '--hook-cmd') options.hookCmd = needsValue();
    else if (arg === '--compare') options.compare = needsValue();
    else if (arg === '--tolerance') {
      const parsed = Number(needsValue());
      if (!Number.isFinite(parsed) || parsed < 0) throw new Error(`--tolerance braucht eine Zahl ab 0, nicht „${value}“.`);
      options.tolerance = parsed;
    } else if (NUMBER_ARGS[arg]) {
      const parsed = Number(needsValue());
      if (!Number.isInteger(parsed) || parsed < 1) throw new Error(`${arg} braucht eine ganze Zahl ab 1, nicht „${value}“.`);
      numbers[NUMBER_ARGS[arg]] = parsed;
    } else throw new Error(`Unbekanntes Argument „${arg}“. --help zeigt die Möglichkeiten.`);
  }
  return { ...options, ...(options.quick ? QUICK : FULL), ...numbers };
}

// Angaben zur Maschine; sie gehören zu jeder Messung, weil Zahlen ohne sie nichts bedeuten
export function machine() {
  return {
    os: platform(),
    release: release(),
    arch: arch(),
    cpu: cpus()[0]?.model?.trim() || 'unbekannt',
    cpus: cpus().length,
    memGb: round1(totalmem() / 1024 ** 3),
    node: process.version,
  };
}

// Fügt die eigenen Messgrößen in eine vorhandene Datei ein; fremde Einträge (z. B. aus P0-2) bleiben erhalten
export function merge(existing, fresh) {
  const before = existing && typeof existing === 'object' && !Array.isArray(existing) ? existing : {};
  const keep = { ...(before.metrics && typeof before.metrics === 'object' ? before.metrics : {}) };
  for (const key of OWN_METRICS) delete keep[key];
  return { ...before, ...fresh, metrics: { ...keep, ...fresh.metrics } };
}

// Liest eine vorhandene Ergebnisdatei; unlesbare Dateien gelten als nicht vorhanden
function readExisting(file) {
  try {
    return JSON.parse(readFileSync(file, 'utf8'));
  } catch {
    return null;
  }
}

// Liest die Datei des Vergleichs; sie muss lesbar sein (sonst wäre der Vergleich nach der Messung nichts wert)
function readBaseline(file) {
  let text;
  try {
    text = readFileSync(file, 'utf8');
  } catch {
    throw new Error(`Die Vergleichsdatei ${file} lässt sich nicht lesen.`);
  }
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`Die Vergleichsdatei ${file} enthält kein gültiges JSON: ${error.message}`);
  }
}

// Kennzahlen einer Messreihe in der Form, in der sie in contract/baseline.json stehen
function series(values) {
  return { median: round1(median(values)), p95: round1(p95(values)), runs: values.length };
}

// Kaltstarts: je Lauf die Zeit bis zur ersten Antwort von /api/health und bis zur ersten Board-Ansicht
async function measureColdStarts(server, boardId, runs) {
  const health = [];
  const firstLoad = [];
  for (let run = 1; run <= runs; run += 1) {
    await server.start();
    health.push(await server.waitFor('/api/health'));
    firstLoad.push(await server.waitFor(`/api/db/boards/${boardId}/view`));
    await server.stop();
    log(`  Kaltstart ${run}/${runs}: health ${round1(health[run - 1])} ms, Board ${round1(firstLoad[run - 1])} ms`);
  }
  return { health, firstLoad };
}

// Warme Ladezeiten der Board-Ansicht bei laufendem Server und danach der Speicherbedarf im Leerlauf
async function measureWarm(server, boardId, loads) {
  await server.start();
  await server.waitFor('/api/health');
  const path = `/api/db/boards/${boardId}/view`;
  // Die erste Ansicht baut die Board-Laufzeit auf; darauf warten, bevor die Aufwärm-Anfragen zählen
  await server.waitFor(path);
  for (let warmup = 0; warmup < WARMUP_LOADS; warmup += 1) await server.measure(path);
  const values = [];
  for (let load = 1; load <= loads; load += 1) values.push(await server.measure(path));
  log(`  ${loads} Ladevorgänge gemessen (p50 ${round1(median(values))} ms, p95 ${round1(p95(values))} ms)`);
  log(`  ${IDLE_MS / 1000} s Ruhe, dann Speichermessung …`);
  await delay(IDLE_MS);
  const pid = await server.serverPid();
  const rss = pid === null ? null : new ProcessMemory().rssMb(pid);
  log(pid === null ? '  Speicher: Prozessnummer des Servers nicht gefunden' : `  Speicher: ${rss} MB (Prozess ${pid})`);
  return { values, rss };
}

// Hook-Laufzeit je Ereignis: ein eigener Prozess je Aufruf, so wie Claude Code die Hooks startet
function measureHooks(project, port, options) {
  const bench = new HookBench({ ...project.hookCommand(port), tmp: project.base, log });
  const metrics = {};
  for (const [name, kind] of Object.entries(HOOK_METRICS)) {
    const result = (options.hyperfine ? bench.hyperfine(kind, options.hookRuns) : null) ?? bench.measure(kind, options.hookRuns);
    metrics[name] = result;
    log(`  ${kind}: Median ${result.median} ms, p95 ${result.p95} ms (${result.runs} Läufe)`);
  }
  return metrics;
}

// Latenz des Ereignisstroms: sessions (sofort über den EventHub) und dbboard (über den Abfrage-Takt des Servers)
async function measureSse(server, project, boardId, ticketId, options) {
  const bench = new SseBench({
    url: server.url, projectId: project.projectId, repo: project.repo, token: project.token, boardId, log,
  });
  const sessions = series(await bench.sessions(options.sseRuns));
  log(`  sessions: Median ${sessions.median} ms, p95 ${sessions.p95} ms`);
  const dbboard = series(await bench.dbBoard(options.dbRuns, ticketId));
  log(`  dbboard: Median ${dbboard.median} ms, p95 ${dbboard.p95} ms`);
  return { sseSessionsLatencyMs: sessions, sseDbBoardLatencyMs: dbboard };
}

// Ganze Messung: befüllen, Testprojekt einrichten, Kaltstarts, warme Ladezeiten, Speicher, Hooks, Ereignisstrom
async function run(options) {
  if (options.serverCmd === DEFAULT_SERVER_CMD && !existsSync(DEFAULT_ENTRY)) {
    throw new Error(`${DEFAULT_ENTRY} fehlt – bitte zuerst „cargo build --release -p nexus-cli --bin nexus“ ausführen (oder --server-cmd angeben).`);
  }
  const home = mkdtempSync(join(tmpdir(), 'harness-bench-'));
  const server = new ServerProcess({ cmd: options.serverCmd, home });
  const project = new BenchProject({ cloneDir: process.cwd(), home, hookCmd: options.hookCmd, log });
  try {
    log(`Messordner ${home}`);
    log(`Befülle das Board mit ${options.tickets} Tickets …`);
    await server.start();
    await server.waitFor('/api/health');
    const seeder = new BoardSeeder(server.url, { ...BENCH_USER, dbFile: join(home, 'board.db'), log });
    const { boardId, firstTicket } = await seeder.seed(options.tickets);

    // Vor den langen Messreihen: Plugin-Bündel, Testprojekt und Zuordnung – so fällt ein fehlender Build früh auf
    log('Richte das Testprojekt der Hooks ein …');
    await project.prepare(server.url, boardId);
    await server.stop();

    log(`Messe ${options.runs} Kaltstarts …`);
    const cold = await measureColdStarts(server, boardId, options.runs);

    log(`Messe ${options.loads} warme Ladevorgänge …`);
    const warm = await measureWarm(server, boardId, options.loads);

    // Der Server läuft weiter: die Hooks melden sich bei ihm, und der Ereignisstrom braucht ihn ohnehin
    log(`Messe die Laufzeit der Hooks (${options.hookRuns} Läufe je Ereignis) …`);
    const hooks = measureHooks(project, server.port, options);

    log('Messe die Latenz des Ereignisstroms …');
    const sse = await measureSse(server, project, boardId, firstTicket, options);

    return {
      schema: SCHEMA,
      createdAt: new Date().toISOString(),
      machine: machine(),
      server: { cmd: options.serverCmd, tickets: options.tickets },
      hooks: { cmd: project.hookDescription() },
      metrics: {
        serverColdStartMs: { median: round1(median(cold.health)), p95: round1(p95(cold.health)), runs: options.runs },
        boardFirstLoadMs: { median: round1(median(cold.firstLoad)), p95: round1(p95(cold.firstLoad)), runs: options.runs },
        boardLoadMs: {
          p50: round1(median(warm.values)),
          p95: round1(p95(warm.values)),
          mean: round1(mean(warm.values)),
          runs: options.loads,
        },
        serverRssIdleMb: { value: warm.rss },
        ...hooks,
        ...sse,
      },
    };
  } finally {
    // Auch nach einem Fehler bleibt kein Serverprozess, kein Testprojekt und kein Messordner zurück
    await server.stop().catch(() => {});
    project.dispose();
    rmSync(home, { recursive: true, force: true });
  }
}

// Vergleich mit einer früheren Messung: Tabelle auf stderr, Rückgabecode 1, wenn ein Wert die Toleranz überschreitet
function compare(baseline, result, tolerance) {
  const rows = compareMetrics(baseline.metrics, result.metrics, tolerance);
  const warning = machineWarning(baseline.machine, result.machine);
  log(`\nVergleich mit der früheren Messung (Toleranz ${tolerance} %, maßgeblich ist p95, sonst median, sonst value):`);
  if (warning) log(warning);
  log(formatComparison(rows, tolerance));
  const worse = worseRows(rows);
  const missing = rows.filter((row) => row.fehlt);
  if (missing.length) log(`Warnung: ${missing.length} Messgröße(n) stehen nur in einer der beiden Messungen.`);
  log(worse.length ? `${worse.length} Messgröße(n) sind schlechter geworden: ${worse.map((row) => row.name).join(', ')}` : 'Keine Messgröße ist schlechter geworden.');
  return worse.length > 0;
}

// Einstieg: Argumente lesen, messen, Ergebnis auf stdout und bei --out zusätzlich in die Datei
async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    process.stdout.write(HELP);
    return;
  }
  // Die Vergleichsdatei vor der Messung lesen: ein Tippfehler soll nicht erst nach Minuten auffallen
  const baseline = options.compare ? readBaseline(options.compare) : null;
  const result = await run(options);
  // Mit --out steht in der Datei und auf stdout dasselbe: das Ergebnis samt der fremden Einträge der Datei
  const output = options.out ? merge(readExisting(options.out), result) : result;
  if (options.out) {
    mkdirSync(dirname(options.out), { recursive: true });
    writeFileSync(options.out, `${JSON.stringify(output, null, 2)}\n`);
    log(`Ergebnis in ${options.out} geschrieben`);
  }
  process.stdout.write(`${JSON.stringify(output, null, 2)}\n`);
  if (baseline && compare(baseline, result, options.tolerance)) process.exitCode = 1;
}

// Als Programm aufgerufen? Beim Import (Tests) läuft nichts von allein
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    log(`Messung abgebrochen: ${error instanceof Error ? error.message : error}`);
    process.exit(1);
  });
}
