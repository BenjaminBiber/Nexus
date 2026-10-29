// Erzeugt das Golden-Protokoll des TypeScript-Workers (`packages/server/src/dispatch/DispatchWorker.ts`),
// gegen das der Rust-Worker geprüft wird. Zeitstempel und Pfade werden ersetzt, damit zwei Läufe
// vergleichbar sind.
//
// Aufruf (im Projektordner, nach `npm ci`):
//   npx tsx crates/nexus-server/tests/fixtures/golden_run_log.mts <ziel.log> <pfad-zu-nexus-fake-claude>
//
// Das Ergebnis liegt als `run_log_stream.golden.log` neben dieser Datei und ist eingecheckt;
// das Skript muss nur laufen, wenn sich das Protokollformat ändern soll.

import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { HarnessHome, RunStore } from '@harness/core';
import { DispatchWorker } from '../../../../packages/server/src/dispatch/DispatchWorker.js';

const [ziel, claude] = process.argv.slice(2);
if (!ziel || !claude) {
  console.error('Aufruf: golden_run_log.mts <ziel.log> <pfad-zu-nexus-fake-claude>');
  process.exit(2);
}

const dir = mkdtempSync(join(tmpdir(), 'nexus-golden-'));
const home = new HarnessHome({ HARNESS_HOME: dir });
const logFile = join(dir, 'strom.log');
const runs = new RunStore(home);
await runs.create({
  id: 'ARC-0004-x', kind: 'work', ticketId: 'ARC-0001', boardId: 1, title: 'T', agent: 'junior-ARC-0004-x',
  cwd: dir, branch: 'b', target: null,
  run: { profile: 'standard', why: 'Standardprofil standard', model: 'opus', effort: 'high', maxTurns: 80, timeoutMinutes: 1 },
  command: claude, args: ['-p'], prompt: 'Zeile 1\n\nZeile 3', logFile,
  startedAt: '2026-10-29T09:00:00.000Z', pid: null,
});

process.env.FAKE_CLAUDE_MODE = 'stream';
process.env.HARNESS_TICKET = 'ARC-0004';
await new DispatchWorker(home).run('ARC-0004-x');

// Gleiche Ersetzungen wie im Rust-Test: zuerst die Pfade, dann die Zeiten
const normalisiert = readFileSync(logFile, 'utf8')
  .split(claude).join('<claude>')
  .split(dir).join('<cwd>')
  .replace(/\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{3}Z/g, '<iso>')
  .replace(/^\d\d:\d\d:\d\d/gm, '<zeit>');
writeFileSync(ziel, normalisiert);
console.log(`Golden-Protokoll geschrieben: ${ziel}`);
