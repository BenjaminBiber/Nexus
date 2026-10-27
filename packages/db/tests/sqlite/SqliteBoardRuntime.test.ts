import { existsSync } from 'node:fs';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BoardConfigStore, HarnessHome, type BoardConfig } from '@harness/core';
import { BoardRuntime } from '../../src/BoardRuntime.js';

const USER_ID = '11111111-1111-1111-1111-111111111111';

// Vollständige Konfig für SQLite; die MSSQL-Felder bleiben leer und werden vom Anbieter sqlite nicht gelesen.
const config = (file: string): BoardConfig => ({
  provider: 'sqlite', file,
  server: '', port: 1433, database: '', user: '', password: '', encrypt: true, trustServerCertificate: true,
  userId: USER_ID, userName: 'Test', activeBoard: null,
});

describe('BoardRuntime mit SQLite', () => {
  let dir: string;
  let home: HarnessHome;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), 'harness-sqlite-runtime-'));
    home = new HarnessHome({ HARNESS_HOME: dir });
  });
  afterEach(() => rm(dir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 }));

  it('legt in einer leeren Datei das Schema an, trägt den Benutzer ein und liefert nutzbare Dienste', async () => {
    const file = join(dir, 'unter', 'board.db');
    const runtime = await BoardRuntime.connect(config(file), {}, home);
    try {
      expect(existsSync(file)).toBe(true);
      expect(runtime.actor()).toEqual({ userId: USER_ID, name: 'Test' });
      await expect(runtime.services.db.transaction((u) => u.users.byId(USER_ID))).resolves.toMatchObject({ id: USER_ID, name: 'Test' });

      const actor = runtime.actor();
      const project = await runtime.services.projects.create('TST', 'Test', actor);
      expect(project.key).toBe('TST');
      const board = await runtime.services.boards.create('TST', 'Sprint', actor);
      const ticket = await runtime.services.tickets.create(board.id, { title: 'Erstes' }, actor);
      expect(ticket.id).toBe('TST-0001');
    } finally {
      await runtime.close();
    }
  });

  it('öffnet dieselbe Datei erneut mit den gespeicherten Daten', async () => {
    const file = join(dir, 'board.db');
    const first = await BoardRuntime.connect(config(file), {}, home);
    try {
      await first.services.projects.create('TST', 'Test', first.actor());
    } finally {
      await first.close();
    }

    const second = await BoardRuntime.connect(config(file), {}, home);
    try {
      expect((await second.services.projects.list()).map((p) => p.key)).toContain('TST');
    } finally {
      await second.close();
    }
  });

  it('öffnet über board.yaml mit provider sqlite', async () => {
    await new BoardConfigStore(home).write(config(join(dir, 'board.db')));

    const runtime = await BoardRuntime.open(home);
    try {
      expect(await runtime.services.projects.list()).toEqual([]);
    } finally {
      await runtime.close();
    }
  });
});
