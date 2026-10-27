import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { BoardConfig } from '@harness/core';
import { BoardDatabaseFactory } from '../src/BoardDatabaseFactory.js';
import { MssqlBoardDatabase } from '../src/MssqlBoardDatabase.js';
import { SqliteBoardDatabase } from '../src/sqlite/SqliteBoardDatabase.js';

const CONFIG: BoardConfig = {
  provider: 'mssql', file: '', server: '127.0.0.1', port: 1, database: 'AiHarness', user: 'u', password: 'p',
  encrypt: false, trustServerCertificate: true, userId: '11111111-1111-1111-1111-111111111111', userName: 'F', activeBoard: null,
};

describe('BoardDatabaseFactory', () => {
  afterEach(() => vi.restoreAllMocks());

  it('liefert für mssql die SQL-Server-Datenbank', async () => {
    const fake = {} as MssqlBoardDatabase;
    const connect = vi.spyOn(MssqlBoardDatabase, 'connect').mockResolvedValue(fake);

    await expect(BoardDatabaseFactory.open(CONFIG, { connectionTimeoutMs: 1234 })).resolves.toBe(fake);
    expect(connect).toHaveBeenCalledWith(CONFIG, { connectionTimeoutMs: 1234 });
  });

  it('liefert für sqlite die Datei-Datenbank', async () => {
    const connect = vi.spyOn(MssqlBoardDatabase, 'connect').mockResolvedValue({} as MssqlBoardDatabase);
    const dir = await mkdtemp(join(tmpdir(), 'harness-sqlite-factory-'));
    const file = join(dir, 'board.db');
    try {
      const db = await BoardDatabaseFactory.open({ ...CONFIG, provider: 'sqlite', file });
      try {
        expect(db).toBeInstanceOf(SqliteBoardDatabase);
      } finally {
        await db.close();
      }
      expect(connect).not.toHaveBeenCalled();
    } finally {
      await rm(dir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
    }
  });

  it('meldet einen unbekannten Anbieter verständlich', async () => {
    await expect(BoardDatabaseFactory.open({ ...CONFIG, provider: 'oracle' } as unknown as BoardConfig)).rejects.toMatchObject({
      code: 'board-provider-unknown',
      message: expect.stringContaining('oracle'),
    });
  });
});
