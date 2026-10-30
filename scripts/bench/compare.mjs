import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { DEFAULT_TOLERANCE_PCT, compareMetrics, formatComparison, machineWarning, worseRows } from './lib/BaselineCompare.mjs';
import { median, round1 } from './lib/Stats.mjs';

// Prüfskript der Leistungsschwelle: vergleicht frische Messwerte mit den Ausgangswerten eines Runners
// und endet mit Code 1, sobald eine Messgröße um mehr als die Toleranz schlechter ist.

const HELP = `Vergleicht Messwerte mit Ausgangswerten und scheitert bei Rückschritt.

  node scripts/bench/compare.mjs --baseline <datei> --measured <datei> [--measured <datei> …]
                                 [--tolerance ${DEFAULT_TOLERANCE_PCT}]

  --baseline <datei>   Ausgangswerte, z. B. contract/baseline-ci-linux.json
  --measured <datei>   Messwerte eines Laufs; mehrfach genannt zählt je Messgröße der Median
  --ignore <name>      Messgröße, die nur berichtet wird und nie scheitern lässt; mehrfach erlaubt
  --tolerance <zahl>   erlaubter Rückschritt in Prozent (Standard ${DEFAULT_TOLERANCE_PCT})
  --help               zeigt diese Hilfe

Rückgabecode 1, sobald eine Messgröße die Toleranz überschreitet. Messgrößen ohne Ausgangswert
werden gemeldet, lassen den Lauf aber nicht scheitern.
`;

// Meldungen gehen auf stderr, damit stdout frei bleibt
const log = (line) => process.stderr.write(`${line}\n`);

// Liest die Argumente des Prüfskripts
export function parseArgs(argv) {
  const options = { baseline: null, measured: [], ignore: [], tolerance: DEFAULT_TOLERANCE_PCT, help: false };
  let index = 0;
  while (index < argv.length) {
    const arg = argv[index];
    const value = argv[index + 1];
    const needsValue = () => {
      if (value === undefined) throw new Error(`${arg} braucht einen Wert.`);
      index += 2;
      return value;
    };
    if (arg === '--help' || arg === '-h') {
      options.help = true;
      index += 1;
    } else if (arg === '--baseline') options.baseline = needsValue();
    else if (arg === '--measured') options.measured.push(needsValue());
    else if (arg === '--ignore') options.ignore.push(needsValue());
    else if (arg === '--tolerance') {
      const parsed = Number(needsValue());
      if (!Number.isFinite(parsed) || parsed < 0) throw new Error(`--tolerance braucht eine Zahl ab 0, nicht „${value}“.`);
      options.tolerance = parsed;
    } else throw new Error(`Unbekanntes Argument „${arg}“. --help zeigt die Möglichkeiten.`);
  }
  return options;
}

// Liest eine Messdatei; ein Fehler darin ist ein Fehler des Vergleichs und wird nicht übergangen
export function readMeasurement(file) {
  let text;
  try {
    text = readFileSync(file, 'utf8');
  } catch {
    throw new Error(`Die Datei ${file} lässt sich nicht lesen.`);
  }
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`Die Datei ${file} enthält kein gültiges JSON: ${error.message}`);
  }
}

// Fasst mehrere Läufe zu einem zusammen: je Messgröße und Feld der Median über die Wiederholungen.
// So entscheidet nicht ein einzelner Ausreißer eines schwankenden Runners über den Rückgabecode.
export function medianOfRuns(runs) {
  const metrics = {};
  for (const run of runs) {
    const source = run?.metrics && typeof run.metrics === 'object' ? run.metrics : {};
    for (const [name, metric] of Object.entries(source)) {
      if (!metric || typeof metric !== 'object' || Array.isArray(metric)) continue;
      const fields = (metrics[name] ??= {});
      for (const [field, value] of Object.entries(metric)) {
        if (typeof value !== 'number' || !Number.isFinite(value)) continue;
        (fields[field] ??= []).push(value);
      }
    }
  }
  const merged = {};
  for (const [name, fields] of Object.entries(metrics)) {
    merged[name] = Object.fromEntries(Object.entries(fields).map(([field, values]) => [field, round1(median(values))]));
  }
  return { machine: runs[0]?.machine, metrics: merged };
}

// Vergleicht und schreibt die Tabelle; liefert den Rückgabecode (1 bei Rückschritt)
export function compare(baseline, measured, tolerance, write = log, ignore = []) {
  const übergangen = new Set(ignore);
  const rows = compareMetrics(baseline?.metrics, measured?.metrics, tolerance)
    .map((row) => (übergangen.has(row.name) ? { ...row, schlechter: false } : row));
  const warning = machineWarning(baseline?.machine, measured?.machine);
  write(`Vergleich mit den Ausgangswerten (Toleranz ${tolerance} %, maßgeblich ist p95, sonst median, sonst value):`);
  if (warning) write(warning);
  write(formatComparison(rows, tolerance));
  const missing = rows.filter((row) => row.fehlt);
  for (const row of missing) {
    write(`Hinweis: „${row.name}“ hat ${row.fehlt === 'alt' ? 'keinen Ausgangswert' : 'keinen Messwert'} – das lässt den Lauf nicht scheitern.`);
  }
  for (const name of übergangen) {
    const row = rows.find((candidate) => candidate.name === name);
    if (row && row.deltaPct !== null) write(`Übergangen: „${name}“ weicht um ${row.deltaPct > 0 ? '+' : ''}${row.deltaPct} % ab und zählt nicht.`);
  }
  const worse = worseRows(rows);
  for (const row of worse) {
    write(`Rückschritt: ${row.name} alt ${row.alt}, neu ${row.neu} (${row.deltaPct > 0 ? '+' : ''}${row.deltaPct} %).`);
  }
  if (worse.length === 0) {
    write('Keine Messgröße ist um mehr als die Toleranz schlechter geworden.');
    return 0;
  }
  write(`${worse.length} Messgröße(n) sind schlechter als die Ausgangswerte: ${worse.map((row) => row.name).join(', ')}`);
  return 1;
}

// Einstieg: Dateien lesen, zusammenfassen, vergleichen
export function main(argv, write = log) {
  const options = parseArgs(argv);
  if (options.help) {
    process.stdout.write(HELP);
    return 0;
  }
  if (!options.baseline) throw new Error('--baseline fehlt. --help zeigt die Möglichkeiten.');
  if (options.measured.length === 0) throw new Error('--measured fehlt. --help zeigt die Möglichkeiten.');
  const baseline = readMeasurement(options.baseline);
  const measured = medianOfRuns(options.measured.map(readMeasurement));
  if (options.measured.length > 1) write(`${options.measured.length} Läufe zusammengefasst (Median je Messgröße).`);
  return compare(baseline, measured, options.tolerance, write, options.ignore);
}

// Als Programm aufgerufen? Beim Import (Tests) läuft nichts von allein
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    process.exitCode = main(process.argv.slice(2));
  } catch (error) {
    log(`Vergleich abgebrochen: ${error instanceof Error ? error.message : error}`);
    process.exit(1);
  }
}
