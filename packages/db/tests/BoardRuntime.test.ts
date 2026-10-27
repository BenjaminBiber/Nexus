import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BoardConfigStore, HarnessHome, type BoardConfig } from '@harness/core';
import { BoardRuntime } from '../src/BoardRuntime.js';
import type { ManagedBoardDatabase } from '../src/ManagedBoardDatabase.js';
import { REQUIRED_SCHEMA_FEATURES } from '../src/SchemaVersionGuard.js';
import { TestDb } from './helpers/TestDb.js';

describe('BoardRuntime ohne Datenbank', () => {
  let dir: string;
  let home: HarnessHome;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), 'harness-runtime-'));
    home = new HarnessHome({ HARNESS_HOME: dir });
  });
  afterEach(() => rm(dir, { recursive: true, force: true }));

  it('meldet eine fehlende Konfig', async () => {
    await expect(BoardRuntime.open(home)).rejects.toMatchObject({ code: 'board-config-missing' });
  });

  it('meldet einen nicht erreichbaren Server mit Server und Datenbank', async () => {
    await new BoardConfigStore(home).write({
      provider: 'mssql', file: join(dir, 'board.db'),
      server: '127.0.0.1', port: 1, database: 'AiHarness', user: 'u', password: 'p', encrypt: false, trustServerCertificate: true,
      userId: '11111111-1111-1111-1111-111111111111', userName: 'F', activeBoard: null,
    });
    await expect(BoardRuntime.open(home)).rejects.toMatchObject({ code: 'board-db-unreachable', message: expect.stringContaining('127.0.0.1:1') });
  });
});

describe('BoardRuntime mit fehlender Erweiterung', () => {
  const config: BoardConfig = {
    provider: 'mssql', file: 'board.db',
    server: '127.0.0.1', port: 1, database: 'AiHarness', user: 'u', password: 'p', encrypt: false, trustServerCertificate: true,
    userId: '11111111-1111-1111-1111-111111111111', userName: 'F', activeBoard: null,
  };

  // Datenbank-Ersatz: liefert die vorgegebenen Erweiterungen; einspielen ergibt die, die nach dem ersten Versuch dazukommen
  const fake = (features: string[], version: number | null = 3, applies: string[] = []) => {
    const calls: { ensured: string[][]; missing: string[][] } = { ensured: [], missing: [] };
    let current = features;
    const db = {
      schemaVersion: async () => version,
      schemaFeatures: async () => current,
      ensureSchemaFeatures: async (missing: string[]) => {
        calls.ensured.push(missing);
        current = [...current, ...applies];
        return applies;
      },
      setMissingFeatures: (names: string[]) => calls.missing.push(names),
      transaction: async (work: (u: unknown) => Promise<unknown>) => work({ users: { upsert: async () => {} } }),
      close: async () => {},
    } as unknown as ManagedBoardDatabase;
    return { calls, open: async () => db };
  };

  // Alle verlangten Erweiterungen; aus dem Guard gelesen, damit eine weitere Stufe diese Tests nicht erneut bricht
  const alle = Object.keys(REQUIRED_SCHEMA_FEATURES);

  it('spielt die fehlende Erweiterung selbst ein und läuft danach ohne Einschränkung', async () => {
    const f = fake([], 3, alle);
    const runtime = await BoardRuntime.connect(config, {}, new HarnessHome(), f.open);
    expect(f.calls.ensured).toEqual([alle]);
    expect(f.calls.missing).toEqual([[]]);
    expect(runtime.missingFeatures).toEqual([]);
  });

  it('verbindet auch ohne DDL-Recht und merkt sich, was weiter fehlt', async () => {
    const f = fake([alle[0]], 3, []);
    const runtime = await BoardRuntime.connect(config, {}, new HarnessHome(), f.open);
    expect(f.calls.ensured).toEqual([alle.slice(1)]);
    expect(runtime.missingFeatures).toEqual(alle.slice(1));
    expect(f.calls.missing).toEqual([alle.slice(1)]);
  });

  it('versucht nichts einzuspielen, wenn alles da ist', async () => {
    const f = fake(alle);
    expect((await BoardRuntime.connect(config, {}, new HarnessHome(), f.open)).missingFeatures).toEqual([]);
    expect(f.calls.ensured).toEqual([]);
  });

  it('bricht bei falscher Schema-Version weiter mit Hinweis auf das Skript ab', async () => {
    const f = fake(alle, 2);
    await expect(BoardRuntime.connect(config, {}, new HarnessHome(), f.open)).rejects.toMatchObject({
      code: 'board-schema-outdated', message: expect.stringContaining('003_'),
    });
    expect(f.calls.ensured).toEqual([]);
  });
});

TestDb.describe('BoardRuntime mit Test-DB', () => {
  beforeEach(() => TestDb.reset());

  it('legt den Benutzer an und liefert Dienste und Handelnden', async () => {
    const runtime = await BoardRuntime.connect(TestDb.config()!);
    try {
      expect(runtime.actor()).toEqual({ userId: '11111111-1111-1111-1111-111111111111', name: 'Test' });
      expect(runtime.actor(' claude ')).toEqual({ userId: '11111111-1111-1111-1111-111111111111', name: 'claude' });
      const project = await runtime.services.projects.create('TST', 'Test', runtime.actor());
      expect(project.key).toBe('TST');
    } finally {
      await runtime.close();
    }
  });
});
