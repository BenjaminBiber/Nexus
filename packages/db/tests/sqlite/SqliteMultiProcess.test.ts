import { BoardServices } from '@harness/core';
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { SqliteBoardDatabase } from '../../src/sqlite/SqliteBoardDatabase.js';

const ROOT = resolve(__dirname, '../../../..');
const WORKER = join(__dirname, 'helpers/multiProcessWorker.ts');
const TSX_LOADER = pathToFileURL(join(ROOT, 'node_modules/tsx/dist/esm/index.mjs')).href;
const TSCONFIG = join(ROOT, 'tsconfig.dev.json');

const ACTOR = { userId: '11111111-1111-1111-1111-111111111111', name: 'Test' };

interface CreateResult { ids: string[]; errors: string[] }
interface UpdateResult { result: string }

// Startet den Hilfsprozess und liefert sein JSON; Exit ≠ 0 lässt den Test mit stderr scheitern
function runWorker<T>(args: string[]): Promise<T> {
  return new Promise((done, fail) => {
    const child = spawn(process.execPath, ['--import', TSX_LOADER, WORKER, ...args], {
      stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true, env: { ...process.env, TSX_TSCONFIG_PATH: TSCONFIG },
    });
    let out = '';
    let err = '';
    child.stdout.on('data', (d) => (out += d));
    child.stderr.on('data', (d) => (err += d));
    child.on('close', (code) => (code === 0 ? done(JSON.parse(out) as T) : fail(new Error(err))));
  });
}

describe('Mehrere Prozesse auf derselben SQLite-Datei', () => {
  let dir: string;
  let file: string;
  let boardId: number;

  // Öffnet die Datei im Testprozess, führt etwas aus und schließt wieder – die Kindprozesse brauchen sie frei
  async function withDatabase<T>(work: (services: BoardServices, db: SqliteBoardDatabase) => Promise<T>): Promise<T> {
    const db = await SqliteBoardDatabase.open(file);
    try {
      return await work(new BoardServices(db), db);
    } finally {
      await db.close();
    }
  }

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), 'harness-multiproc-'));
    file = join(dir, 'board.db');
    boardId = await withDatabase(async (services, db) => {
      await db.transaction((u) => u.users.upsert({ id: ACTOR.userId, name: ACTOR.name }));
      await services.projects.create('ARC', 'Arches', ACTOR);
      return (await services.boards.create('ARC', 'Sprint', ACTOR)).id;
    });
  });

  afterEach(async () => { await rm(dir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 }); });

  it('legt aus zwei Prozessen je 50 Tickets an, ohne Lücke und ohne Fehler', async () => {
    const [a, b] = await Promise.all([
      runWorker<CreateResult>(['create', file, String(boardId), '50', 'A']),
      runWorker<CreateResult>(['create', file, String(boardId), '50', 'B']),
    ]);

    expect([...a.errors, ...b.errors]).toEqual([]);
    expect(a.ids).toHaveLength(50);
    expect(b.ids).toHaveLength(50);

    await withDatabase(async (services) => {
      const tickets = await services.tickets.list({ boardIds: [boardId] });
      expect(tickets).toHaveLength(100);
      const numbers = tickets.map((t) => Number(t.id.slice(4))).sort((x, y) => x - y);
      expect(numbers).toEqual(Array.from({ length: 100 }, (_, i) => i + 1));
      expect(new Set(tickets.map((t) => t.id))).toEqual(new Set([...a.ids, ...b.ids]));
      expect((await services.projects.list())[0].nextTicketNumber).toBe(101);
    });
  });

  it('ändert dieselbe Rev aus zwei Prozessen: genau einer gewinnt, der andere bekommt conflict', async () => {
    const id = await withDatabase(async (services) => (await services.tickets.create(boardId, { title: 'Start' }, ACTOR)).id);

    const results = await Promise.all([
      runWorker<UpdateResult>(['update', file, id, '1', 'Von A']),
      runWorker<UpdateResult>(['update', file, id, '1', 'Von B']),
    ]);

    expect(results.map((r) => r.result).sort()).toEqual(['conflict', 'ok']);
    const winner = results[0].result === 'ok' ? 'Von A' : 'Von B';
    await withDatabase(async (services) => {
      const ticket = await services.tickets.get(id);
      expect(ticket.rev).toBe(2);
      expect(ticket.title).toBe(winner);
    });
  });
});
