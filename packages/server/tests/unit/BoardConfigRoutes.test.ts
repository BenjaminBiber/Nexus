import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { TestServer } from '../helpers/TestServer.js';

const INPUT = {
  server: '127.0.0.1', port: 1, database: 'AiHarness', user: 'harness', password: 'geheim',
  encrypt: false, trustServerCertificate: true, userId: '11111111-1111-1111-1111-111111111111', userName: 'Florian',
};

const SQLITE = { provider: 'sqlite', userId: '11111111-1111-1111-1111-111111111111', userName: 'Florian' };

describe('BoardConfigRoutes', () => {
  let server: TestServer;

  beforeEach(async () => { server = await TestServer.start(); });
  afterEach(() => server.dispose());

  it('liefert ohne Datei Standardwerte und eine Vorschlags-GUID', async () => {
    const res = await server.get('/api/board-config');
    expect(res.status).toBe(200);
    expect(res.json).toMatchObject({
      configured: false, port: 1433, database: 'AiHarness', hasPassword: false,
      provider: 'mssql', dbFile: join(server.home.dir, 'board.db'), file: server.home.boardConfigFile,
    });
    expect(res.json.userId).toMatch(/^[0-9a-f-]{36}$/);
  });

  it('speichert, zeigt das Passwort nie und behält es bei leerem Feld', async () => {
    const saved = await server.put('/api/board-config', INPUT);
    expect(saved.status).toBe(200);
    expect(saved.json).toMatchObject({ configured: true, server: '127.0.0.1', hasPassword: true });
    expect(saved.json).not.toHaveProperty('password');
    // file im DTO ist der Pfad der board.yaml, nicht der der SQLite-Datei
    expect(saved.json).toMatchObject({ provider: 'mssql', file: server.home.boardConfigFile });
    await server.put('/api/board-config', { ...INPUT, password: '', userName: 'Flo' });
    const yaml = readFileSync(server.home.boardConfigFile, 'utf8');
    expect(yaml).toContain('password: geheim');
    expect(yaml).toContain('userName: Flo');
  });

  it('lehnt eine ungültige Konfig mit 422 ab', async () => {
    const res = await server.put('/api/board-config', { ...INPUT, userId: 'kaputt' });
    expect(res.status).toBe(422);
    expect(res.json.message).toContain('GUID');
  });

  it('meldet beim Verbindungstest einen nicht erreichbaren Server, ohne zu speichern', async () => {
    const res = await server.post('/api/board-config/test', INPUT);
    expect(res.status).toBe(200);
    expect(res.json).toMatchObject({ ok: false, message: expect.stringContaining('nicht erreichbar') });
    expect((await server.get('/api/board-config')).json.configured).toBe(false);
  });

  it('speichert eine sqlite-Konfig und liest sie wieder', async () => {
    const dbFile = join(server.home.dir, 'daten', 'b.db');
    const saved = await server.put('/api/board-config', { ...SQLITE, dbFile });
    expect(saved.status).toBe(200);
    expect(saved.json).toMatchObject({ configured: true, provider: 'sqlite', dbFile, file: server.home.boardConfigFile });
    expect((await server.get('/api/board-config')).json).toMatchObject({ configured: true, provider: 'sqlite', dbFile });
    expect(readFileSync(server.home.boardConfigFile, 'utf8')).toContain('provider: sqlite');
  });

  it('übernimmt ein mitgeschicktes file nicht als Datenbankpfad', async () => {
    const saved = await server.put('/api/board-config', { ...SQLITE, file: 'X:/falsch.yaml' });
    expect(saved.status).toBe(200);
    expect(saved.json.dbFile).toBe(join(server.home.dir, 'board.db'));
  });

  it('lehnt ein leeres dbFile bei sqlite mit 422 ab', async () => {
    const res = await server.put('/api/board-config', { ...SQLITE, dbFile: '' });
    expect(res.status).toBe(422);
    expect(res.json.message).toContain('file');
  });

  it('behält SQL-Server-Felder beim Wechsel auf sqlite', async () => {
    await server.put('/api/board-config', INPUT);
    const saved = await server.put('/api/board-config', { ...INPUT, ...SQLITE, password: '' });
    expect(saved.json).toMatchObject({ provider: 'sqlite', server: '127.0.0.1', hasPassword: true });
  });

  it('meldet beim Verbindungstest gegen eine neue Temp-Datei Erfolg, ohne zu speichern', async () => {
    const dbFile = join(server.home.dir, 'probe', 'board.db');
    const res = await server.post('/api/board-config/test', { ...SQLITE, dbFile });
    expect(res.json).toMatchObject({ ok: true, message: expect.stringContaining('Datei geöffnet') });
    expect(existsSync(dbFile)).toBe(true);
    expect((await server.get('/api/board-config')).json.configured).toBe(false);
  });

  it('meldet beim Verbindungstest auf einen Ordner statt einer Datei einen Fehler', async () => {
    const res = await server.post('/api/board-config/test', { ...SQLITE, dbFile: server.home.dir });
    expect(res.json.ok).toBe(false);
    expect(res.json.message).not.toBe('');
  });

  it('repariert eine kaputte board.yaml per PUT, GET meldet den Fehler vorher weiter', async () => {
    mkdirSync(dirname(server.home.boardConfigFile), { recursive: true });
    writeFileSync(server.home.boardConfigFile, 'server: s\n');
    expect((await server.get('/api/board-config')).status).toBe(422);
    const saved = await server.put('/api/board-config', INPUT);
    expect(saved.status).toBe(200);
    expect(saved.json).toMatchObject({ configured: true, server: '127.0.0.1', hasPassword: true });
    expect((await server.get('/api/board-config')).json).toMatchObject({ configured: true, userName: 'Florian' });
  });
});
