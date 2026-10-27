import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { arch, cpus, platform, release, tmpdir, totalmem } from 'node:os';
import { dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { BoardSeeder } from './lib/BoardSeeder.mjs';
import { ProcessMemory } from './lib/ProcessMemory.mjs';
import { ServerProcess } from './lib/ServerProcess.mjs';
import { mean, median, p95, round1 } from './lib/Stats.mjs';

// Version des Ausgabeformats von contract/baseline.json
const SCHEMA = 1;
// Startbefehl des heutigen TypeScript-Servers; `{port}` wird durch den freien Port ersetzt
const DEFAULT_SERVER_CMD = 'node packages/server/dist/main.js --port {port}';
// Programmdatei, die der Standardbefehl braucht (nur dann geprüft)
const DEFAULT_ENTRY = join('packages', 'server', 'dist', 'main.js');
// Umfang einer vollen Messung und der schnellen Messung (--quick, für Tests und Rauchtest)
const FULL = { tickets: 500, runs: 10, loads: 100 };
const QUICK = { tickets: 50, runs: 3, loads: 10 };
// Anfragen vor der warmen Messreihe, damit nicht der erste Zugriff die Werte prägt
const WARMUP_LOADS = 5;
// Ruhezeit vor der Speichermessung
const IDLE_MS = 5000;
// Benutzer der Messung (board.yaml braucht eine GUID und einen Namen)
const BENCH_USER = { userId: '11111111-1111-1111-1111-111111111111', userName: 'Bench' };
// Messgrößen, die dieses Skript schreibt; fremde Einträge in der Datei bleiben unberührt
const OWN_METRICS = ['serverColdStartMs', 'boardFirstLoadMs', 'boardLoadMs', 'serverRssIdleMb'];

const HELP = `Messskript für die Ausgangswerte der Migration.

  node scripts/bench/bench.mjs [--out <datei>] [--server-cmd "<befehl mit {port}>"]
                               [--tickets ${FULL.tickets}] [--runs ${FULL.runs}] [--loads ${FULL.loads}] [--quick]

  --out <datei>       schreibt das Ergebnis zusätzlich in diese Datei (z. B. contract/baseline.json)
  --server-cmd <text> Startbefehl des zu messenden Servers; {port} wird ersetzt
                      (Standard: ${DEFAULT_SERVER_CMD})
  --tickets <zahl>    Tickets, mit denen das Board befüllt wird
  --runs <zahl>       Zahl der gemessenen Kaltstarts
  --loads <zahl>      Zahl der gemessenen warmen Ladevorgänge
  --quick             kleine Messung (${QUICK.tickets} Tickets, ${QUICK.runs} Starts, ${QUICK.loads} Ladevorgänge)
`;

// Wartet eine Weile
const delay = (ms) => new Promise((done) => setTimeout(done, ms));
// Fortschritt geht auf stderr, damit auf stdout nur das JSON steht
const log = (line) => process.stderr.write(`${line}\n`);

// Namen der Zahlenargumente auf ihre Felder
const NUMBER_ARGS = { '--tickets': 'tickets', '--runs': 'runs', '--loads': 'loads' };

// Liest die Argumente; ausdrücklich genannte Zahlen gelten auch zusammen mit --quick
export function parseArgs(argv) {
  const options = { out: null, serverCmd: DEFAULT_SERVER_CMD, help: false, quick: argv.includes('--quick') };
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
    if (arg === '--quick') index += 1;
    else if (arg === '--help' || arg === '-h') {
      options.help = true;
      index += 1;
    } else if (arg === '--out') options.out = needsValue();
    else if (arg === '--server-cmd') options.serverCmd = needsValue();
    else if (NUMBER_ARGS[arg]) {
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

// Ganze Messung: befüllen, Kaltstarts, warme Ladezeiten, Speicher
async function run(options) {
  if (options.serverCmd === DEFAULT_SERVER_CMD && !existsSync(DEFAULT_ENTRY)) {
    throw new Error(`${DEFAULT_ENTRY} fehlt – bitte zuerst „npm run build“ ausführen (oder --server-cmd angeben).`);
  }
  const home = mkdtempSync(join(tmpdir(), 'harness-bench-'));
  const server = new ServerProcess({ cmd: options.serverCmd, home });
  try {
    log(`Messordner ${home}`);
    log(`Befülle das Board mit ${options.tickets} Tickets …`);
    await server.start();
    await server.waitFor('/api/health');
    const seeder = new BoardSeeder(server.url, { ...BENCH_USER, dbFile: join(home, 'board.db'), log });
    const { boardId } = await seeder.seed(options.tickets);
    await server.stop();

    log(`Messe ${options.runs} Kaltstarts …`);
    const cold = await measureColdStarts(server, boardId, options.runs);

    log(`Messe ${options.loads} warme Ladevorgänge …`);
    const warm = await measureWarm(server, boardId, options.loads);

    return {
      schema: SCHEMA,
      createdAt: new Date().toISOString(),
      machine: machine(),
      server: { cmd: options.serverCmd, tickets: options.tickets },
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
      },
    };
  } finally {
    // Auch nach einem Fehler bleibt kein Serverprozess und kein Messordner zurück
    await server.stop().catch(() => {});
    rmSync(home, { recursive: true, force: true });
  }
}

// Einstieg: Argumente lesen, messen, Ergebnis auf stdout und bei --out zusätzlich in die Datei
async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    process.stdout.write(HELP);
    return;
  }
  const result = await run(options);
  // Mit --out steht in der Datei und auf stdout dasselbe: das Ergebnis samt der fremden Einträge der Datei
  const output = options.out ? merge(readExisting(options.out), result) : result;
  if (options.out) {
    mkdirSync(dirname(options.out), { recursive: true });
    writeFileSync(options.out, `${JSON.stringify(output, null, 2)}\n`);
    log(`Ergebnis in ${options.out} geschrieben`);
  }
  process.stdout.write(`${JSON.stringify(output, null, 2)}\n`);
}

// Als Programm aufgerufen? Beim Import (Tests) läuft nichts von allein
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    log(`Messung abgebrochen: ${error instanceof Error ? error.message : error}`);
    process.exit(1);
  });
}
