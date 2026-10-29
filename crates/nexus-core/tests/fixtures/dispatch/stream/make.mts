// Erzeugt die Golden-Fixture der Stream-Auswertung mit dem TypeScript-Original.
// Einmal ausführen, roh.jsonl und protokoll.txt einchecken; der Rust-Test
// `tests/stream_protokoll_mit_ts.rs` schickt roh.jsonl durch `StreamEventFormatter`
// und vergleicht die Protokollzeilen Byte für Byte.
//
//   npx tsx crates/nexus-core/tests/fixtures/dispatch/stream/make.mts
//
// Die Uhrzeit steht fest (siehe AT), damit `stamped` überall denselben Text ergibt;
// die Zeitzone ist UTC, weil `stamped` die Ortszeit nimmt.
process.env.TZ = 'UTC';

import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { StreamEventFormatter } from '../../../../../../packages/core/src/dispatch/StreamEventFormatter.js';

const here = dirname(fileURLToPath(import.meta.url));

// Arbeitsordner des Laufs; Pfade darin erscheinen relativ
const ROOT = '/work/Nexus-worktrees/ARC-0001';
// Feste Uhrzeit der Protokollzeilen
const AT = new Date('2026-10-29T13:45:07.000Z');

// Die Ereignisse der Fake-Ausgabe aus packages/server/tests/helpers/FakeClaude.ts (Modus „stream“),
// dahinter weitere echte Ereignisse, damit alle Arten einer Protokollzeile vorkommen
const EVENTS: unknown[] = [
  { type: 'system', subtype: 'init', model: 'fake-opus' },
  {
    type: 'assistant',
    message: {
      content: [
        { type: 'text', text: 'Ich lese das Ticket.' },
        { type: 'tool_use', id: 't1', name: 'Bash', input: { command: 'harness board show ARC-0001' } },
      ],
    },
  },
  { type: 'user', message: { content: [{ type: 'tool_result', tool_use_id: 't1', is_error: true, content: 'abgelehnt' }] } },
  { type: 'assistant', message: { content: [{ type: 'tool_use', id: 't2', name: 'Read', input: { file_path: `${ROOT}/src/Login.ts`, offset: 20, limit: 40 } }] } },
  { type: 'user', message: { content: [{ type: 'tool_result', tool_use_id: 't2', content: [{ type: 'text', text: 'export class Login {}' }] }] } },
  { type: 'assistant', message: { content: [{ type: 'tool_use', id: 't3', name: 'Edit', input: { file_path: `${ROOT}/src/Login.ts` } }] } },
  { type: 'user', message: { content: [{ type: 'tool_result', tool_use_id: 't3', content: 'ok' }] } },
  { type: 'assistant', message: { content: [{ type: 'tool_use', id: 't4', name: 'Grep', input: { pattern: 'login', path: `${ROOT}/src`, glob: '*.ts' } }] } },
  { type: 'assistant', message: { content: [{ type: 'tool_use', id: 't5', name: 'TodoWrite', input: { todos: [{ status: 'completed', content: 'Quellen lesen' }, { status: 'in_progress', content: 'Portieren' }, { status: 'pending', content: 'Tests' }] } }] } },
  { type: 'assistant', message: { content: [{ type: 'tool_use', id: 't6', name: 'Bash', input: { command: 'npm test', description: 'Unit-Tests ausführen' } }] } },
  { type: 'user', message: { content: [{ type: 'tool_result', tool_use_id: 't6', is_error: true, content: ' Tests  3 failed | 409 passed\n' }] } },
  { type: 'unbekannt', foo: 1 },
  { type: 'result', subtype: 'success', is_error: false, num_turns: 3, duration_ms: 60000, result: 'erledigt', permission_denials: [{ tool_name: 'Bash', tool_input: { command: 'git push' } }] },
];

const raw = EVENTS.map((e) => JSON.stringify(e) + '\n').join('')
  // eine Zeile ohne JSON, wie sie Claude Code gelegentlich auf stderr/stdout schreibt
  .replace('{"type":"unbekannt","foo":1}\n', 'Hinweis: alles in Ordnung\n{"type":"unbekannt","foo":1}\n');

// Absichtlich mitten in einer Zeile geteilt, wie bei FakeClaude; die Grenzen stehen in der Fixture nicht,
// der Rust-Test teilt selbst an mehreren Stellen und muss dasselbe Ergebnis bekommen.
const formatter = new StreamEventFormatter({ root: ROOT });
const half = Math.floor(raw.length / 2);
const lines = [...formatter.push(raw.slice(0, half)), ...formatter.push(raw.slice(half)), ...formatter.flush()];
const protokoll = lines.map((e) => formatter.stamped(e, AT)).join('');

writeFileSync(join(here, 'roh.jsonl'), raw, 'utf8');
writeFileSync(join(here, 'protokoll.txt'), protokoll, 'utf8');
console.log(`roh.jsonl: ${raw.length} Zeichen, protokoll.txt: ${lines.length} Zeilen`);
