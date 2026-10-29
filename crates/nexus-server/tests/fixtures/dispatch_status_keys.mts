// Erzeugt die Schlüssel der JSON-Form von `DispatchStatus` & Co. aus
// `packages/server/src/dispatch/DispatchStatus.ts`, gegen die der Rust-Zustand geprüft wird.
//
// Die Objekte unten sind mit den TypeScript-Typen annotiert: fehlt ein Feld oder ist eines zu
// viel, scheitert schon `npm run typecheck`. Das Skript schreibt nur noch deren Schlüssel
// heraus, damit der Rust-Test sie vergleichen kann.
//
// Aufruf (im Projektordner, nach `npm ci`):
//   npx tsx crates/nexus-server/tests/fixtures/dispatch_status_keys.mts
//
// Das Ergebnis liegt als `dispatch_status_keys.golden.json` neben dieser Datei und ist
// eingecheckt; das Skript muss nur laufen, wenn sich die JSON-Form ändern soll.

import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { RunLogParts } from '@harness/core';
import type {
  DispatchMergeInfo,
  DispatchPlanningInfo,
  DispatchResearchInfo,
  DispatchRunInfo,
  DispatchRunLog,
  DispatchStatus,
  DispatchTicketRow,
  QuestionResearchStatus,
  TicketPlanningStatus,
} from '../../../../packages/server/src/dispatch/DispatchStatus.js';

const run: DispatchRunInfo = {
  id: 'ARC-0001-20261029-090000', kind: 'work', ticketId: 'ARC-0001', title: 'T',
  profile: 'standard', model: 'opus', effort: 'high', startedAt: '2026-10-29T09:00:00.000Z',
  log: 'ARC-0001-20261029-090000.log',
};

const research: DispatchResearchInfo = {
  auto: false, wip: 1, active: 0, maxPerDay: 10, today: 0, since: null, retryAt: null, error: null,
};

const planning: DispatchPlanningInfo = {
  auto: false, wip: 1, active: 0, since: null, retryAt: null, error: null,
};

const merge: DispatchMergeInfo = { ticketId: 'ARC-0001', phase: 'waiting', branch: 'b', target: null };

const row: DispatchTicketRow = { id: 'ARC-0001', title: 'T', reason: 'bereit' };

const runLog: DispatchRunLog = {
  name: 'ARC-0001-20261029-090000.log', kind: 'work', started: '2026-10-29T09:00:00.000Z', running: true,
};

const status: DispatchStatus = {
  enabled: true, state: 'idle', message: 'bereit · 0/1', pausedUntil: null, wip: 1, active: 0,
  planning: 0, runs: [run], research, planningAuto: planning, merges: [merge], boardId: 1,
  boardInScope: true, tickets: [row], ticketsError: null, lastTick: null, lastErrors: [],
  configFile: 'dispatch.yaml', configError: null, agentPrefix: 'junior-',
};

const questionStatus: QuestionResearchStatus = {
  questionId: 'ARC-F0001', run: null, pausedUntil: null, lastStartedAt: null, count: 0,
};

const planStatus: TicketPlanningStatus = { ticketId: 'ARC-0001', run: null, pausedUntil: null };

// `runLog` der Route ist `RunLogParts` plus dem Dateinamen
const runLogText: RunLogParts & { name: string } = {
  name: 'ARC-0001-20261029-090000.log', header: 'h', prompt: 'p', body: 'b',
};

const golden = {
  dispatchStatus: Object.keys(status),
  dispatchRunInfo: Object.keys(run),
  dispatchResearchInfo: Object.keys(research),
  dispatchPlanningInfo: Object.keys(planning),
  dispatchMergeInfo: Object.keys(merge),
  dispatchTicketRow: Object.keys(row),
  dispatchRunLog: Object.keys(runLog),
  dispatchRunLogText: Object.keys(runLogText),
  questionResearchStatus: Object.keys(questionStatus),
  ticketPlanningStatus: Object.keys(planStatus),
};

const ziel = join(import.meta.dirname, 'dispatch_status_keys.golden.json');
writeFileSync(ziel, `${JSON.stringify(golden, null, 2)}\n`);
console.log(`geschrieben: ${ziel}`);
