import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BoardConfigStore } from '../../../src/board/BoardConfigStore.js';
import { HarnessHome } from '../../../src/registry/HarnessHome.js';

const GUID = '11111111-1111-1111-1111-111111111111';

const VALID = {
  provider: 'mssql', file: join(tmpdir(), 'harness-board.db'),
  server: 'dev-sql-5', port: 1433, database: 'AiHarness', user: 'harness', password: 'geheim',
  encrypt: true, trustServerCertificate: true,
  userId: GUID, userName: 'Florian', activeBoard: null,
};

describe('BoardConfigStore', () => {
  let dir: string;
  let store: BoardConfigStore;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), 'harness-boardcfg-'));
    store = new BoardConfigStore(new HarnessHome({ HARNESS_HOME: dir }));
  });
  afterEach(() => rm(dir, { recursive: true, force: true }));

  it('liefert null ohne Datei und require nennt den Weg über die Einstellungen', async () => {
    expect(await store.read()).toBeNull();
    await expect(store.require()).rejects.toMatchObject({ code: 'board-config-missing', message: expect.stringContaining('Einstellungen') });
  });

  it('schreibt YAML und liest es wieder', async () => {
    await store.write(VALID);
    expect(await readFile(join(dir, 'board.yaml'), 'utf8')).toContain('server: dev-sql-5');
    expect(await store.read()).toEqual(VALID);
  });

  it('ergänzt Standardwerte für Port, Verschlüsselung und aktives Board', async () => {
    await writeFile(join(dir, 'board.yaml'), 'server: s\ndatabase: d\nuser: u\npassword: p\nuserId: 11111111-1111-1111-1111-111111111111\nuserName: F\n');
    expect(await store.read()).toMatchObject({ port: 1433, encrypt: true, trustServerCertificate: true, activeBoard: null });
  });

  it('nennt fehlende Pflichtfelder', async () => {
    await writeFile(join(dir, 'board.yaml'), 'server: s\n');
    await expect(store.read()).rejects.toMatchObject({
      code: 'board-config-invalid',
      message: expect.stringContaining('database'),
    });
    await expect(store.read()).rejects.toMatchObject({ message: expect.stringContaining('SQL Server') });
  });

  it('lehnt eine ungültige Benutzer-GUID ab', () => {
    expect(() => store.validate({ ...VALID, userId: 'abc' })).toThrow(expect.objectContaining({ code: 'board-config-invalid' }));
  });

  it('lädt eine bestehende board.yaml ohne provider als mssql mit Standarddatei', async () => {
    await writeFile(join(dir, 'board.yaml'), 'server: s\ndatabase: d\nuser: u\npassword: p\nuserId: 11111111-1111-1111-1111-111111111111\nuserName: F\n');
    expect(await store.read()).toMatchObject({ provider: 'mssql', file: join(dir, 'board.db') });
  });

  it('ergibt bei provider sqlite ohne file den Standardpfad unter HARNESS_HOME', () => {
    expect(store.validate({ ...VALID, provider: 'sqlite', file: undefined })).toMatchObject({ provider: 'sqlite', file: join(dir, 'board.db') });
  });

  it('übernimmt einen eigenen Pfad; relative Pfade gelten ab HARNESS_HOME', () => {
    expect(store.validate({ ...VALID, file: join('daten', 'b.db') })).toMatchObject({ file: join(dir, 'daten', 'b.db') });
    expect(store.validate({ ...VALID, file: join(dir, 'x.db') })).toMatchObject({ file: join(dir, 'x.db') });
  });

  it('lehnt einen unbekannten provider ab', () => {
    expect(() => store.validate({ ...VALID, provider: 'oracle' })).toThrow(expect.objectContaining({ code: 'board-config-invalid', message: expect.stringContaining('provider') }));
  });

  it('schreibt provider und file in die YAML', async () => {
    await store.write(VALID);
    expect(await readFile(join(dir, 'board.yaml'), 'utf8')).toContain('provider: mssql');
  });

  it('übernimmt unquotierte Zahlen als Text statt sie als fehlend zu melden', async () => {
    await writeFile(join(dir, 'board.yaml'), 'server: 10\ndatabase: 2024\nuser: 42\npassword: 1234\nuserId: 11111111-1111-1111-1111-111111111111\nuserName: 7\n');
    expect(await store.read()).toMatchObject({ server: '10', database: '2024', user: '42', password: '1234', userName: '7' });
  });

  it('sqlite ohne server, database, user und password ist gültig', () => {
    expect(store.validate({ provider: 'sqlite', userId: GUID, userName: 'F' })).toEqual({
      provider: 'sqlite', file: join(dir, 'board.db'),
      server: '', port: 1433, database: '', user: '', password: '',
      encrypt: true, trustServerCertificate: true,
      userId: GUID, userName: 'F', activeBoard: null,
    });
  });

  it('sqlite mit leerem file ist ungültig', () => {
    for (const file of ['', '   ', 5]) {
      expect(() => store.validate({ provider: 'sqlite', userId: GUID, userName: 'F', file })).toThrow(
        expect.objectContaining({ code: 'board-config-invalid', message: expect.stringContaining('file') }),
      );
    }
  });

  it('sqlite behält vorhandene SQL-Server-Felder', () => {
    expect(store.validate({ ...VALID, provider: 'sqlite' })).toMatchObject({ server: 'dev-sql-5', password: 'geheim' });
  });

  it('sqlite nennt nur die eigenen fehlenden Felder', () => {
    expect(() => store.validate({ provider: 'sqlite', userName: 'F' })).toThrow(
      expect.objectContaining({
        code: 'board-config-invalid',
        message: expect.stringContaining('userId'),
      }),
    );
    try {
      store.validate({ provider: 'sqlite', userName: 'F' });
    } catch (err) {
      expect((err as Error).message).not.toContain('server');
    }
  });

  it('sqlite ignoriert einen ungültigen Port', () => {
    expect(store.validate({ provider: 'sqlite', userId: GUID, userName: 'F', port: 'abc' })).toMatchObject({ port: 1433 });
  });

  it('sqlite-Konfig übersteht Schreiben und Lesen', async () => {
    const config = { ...VALID, provider: 'sqlite' as const, file: join(dir, 'b.db'), server: '', database: '', user: '', password: '' };
    await store.write(config);
    expect(await store.read()).toEqual(config);
  });
});
