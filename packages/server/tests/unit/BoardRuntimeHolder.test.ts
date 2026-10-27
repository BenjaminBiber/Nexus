import { describe, expect, it, vi } from 'vitest';
import { HarnessError } from '@harness/core';
import { BoardRuntimeHolder } from '../../src/dbboard/BoardRuntimeHolder.js';
import type { BoardBackend } from '../../src/dbboard/BoardBackend.js';
import { MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';

const INPUT = {
  server: '127.0.0.1', port: 1, database: 'AiHarness', user: 'harness', password: 'geheim',
  encrypt: false, trustServerCertificate: true, userId: '11111111-1111-1111-1111-111111111111', userName: 'Florian',
};

// node:sqlite-Fehler wie vom Treiber
const sqliteError = (errcode: number, message = 'unable to open database file') =>
  Object.assign(new Error(message), { code: 'ERR_SQLITE_ERROR', errcode, errstr: message });

describe('BoardRuntimeHolder', () => {
  it('baut die Laufzeit erst bei Bedarf und für gleichzeitige Anfragen nur einmal auf', async () => {
    const factory = vi.fn(async (): Promise<BoardBackend> => MemoryBoardBackend.create());
    const holder = new BoardRuntimeHolder(factory);
    expect(factory).not.toHaveBeenCalled();
    const [a, b] = await Promise.all([holder.get(), holder.get()]);
    expect(a).toBe(b);
    expect(factory).toHaveBeenCalledTimes(1);
  });

  it('merkt sich einen fehlgeschlagenen Aufbau nicht', async () => {
    const factory = vi.fn<() => Promise<BoardBackend>>()
      .mockRejectedValueOnce(new HarnessError('board-db-unreachable', 'Server weg'))
      .mockImplementation(async () => MemoryBoardBackend.create());
    const holder = new BoardRuntimeHolder(factory);
    await expect(holder.get()).rejects.toMatchObject({ code: 'board-db-unreachable' });
    await expect(holder.get()).resolves.toBeTruthy();
    expect(factory).toHaveBeenCalledTimes(2);
  });

  it('meldet eine kaputte board.yaml als board-config-broken', async () => {
    const holder = new BoardRuntimeHolder(async () => {
      throw new HarnessError('board-config-invalid', 'board.yaml ist kein gültiges YAML.');
    });
    await expect(holder.get()).rejects.toMatchObject({ code: 'board-config-broken', message: expect.stringContaining('Einstellungen') });
  });

  it('verwirft die Laufzeit nach einem Verbindungsfehler und baut beim nächsten Mal neu auf', async () => {
    const first = await MemoryBoardBackend.create();
    const factory = vi.fn<() => Promise<BoardBackend>>()
      .mockResolvedValueOnce(first)
      .mockImplementation(async () => MemoryBoardBackend.create());
    const holder = new BoardRuntimeHolder(factory);
    const broken = Object.assign(new Error('Socket geschlossen'), { name: 'ConnectionError', code: 'ESOCKET' });
    await expect(holder.run(async () => { throw broken; })).rejects.toMatchObject({ code: 'board-db-unreachable' });
    expect(first.closed).toBe(true);
    expect(await holder.get()).not.toBe(first);
  });

  it('lässt fachliche Fehler durch, ohne neu aufzubauen', async () => {
    const factory = vi.fn(async (): Promise<BoardBackend> => MemoryBoardBackend.create());
    const holder = new BoardRuntimeHolder(factory);
    await expect(holder.run(async () => { throw new HarnessError('title-empty', 'leer'); })).rejects.toMatchObject({ code: 'title-empty' });
    await holder.get();
    expect(factory).toHaveBeenCalledTimes(1);
  });

  it('schließt eine frisch gebaute Laufzeit nicht, wenn ein später Fehler der alten Laufzeit eintrifft', async () => {
    const a = await MemoryBoardBackend.create();
    const b = await MemoryBoardBackend.create();
    const factory = vi.fn<() => Promise<BoardBackend>>().mockResolvedValueOnce(a).mockResolvedValueOnce(b);
    const holder = new BoardRuntimeHolder(factory);
    const broken = Object.assign(new Error('Socket geschlossen'), { name: 'ConnectionError', code: 'ESOCKET' });
    let release!: () => void;
    const gate = new Promise<void>((resolve) => { release = resolve; });
    const first = holder.run(async () => { throw broken; });
    const second = holder.run(async () => { await gate; throw broken; });
    await expect(first).rejects.toMatchObject({ code: 'board-db-unreachable' });
    expect(a.closed).toBe(true);
    expect(await holder.get()).toBe(b);
    release();
    await expect(second).rejects.toMatchObject({ code: 'board-db-unreachable' });
    expect(b.closed).toBe(false);
    expect(factory).toHaveBeenCalledTimes(2);
  });

  it('behandelt einen Abfrage-Timeout (ETIMEOUT) nicht als Verbindungsverlust', async () => {
    const first = await MemoryBoardBackend.create();
    const factory = vi.fn<() => Promise<BoardBackend>>().mockResolvedValue(first);
    const holder = new BoardRuntimeHolder(factory);
    const timeout = Object.assign(new Error('Timeout: Request failed to complete'), { name: 'RequestError', code: 'ETIMEOUT' });
    await expect(holder.run(async () => { throw timeout; })).rejects.toBe(timeout);
    expect(first.closed).toBe(false);
    await holder.get();
    expect(factory).toHaveBeenCalledTimes(1);
  });

  it('schließt bei reset und baut danach neu auf', async () => {
    const first = await MemoryBoardBackend.create();
    const factory = vi.fn<() => Promise<BoardBackend>>()
      .mockResolvedValueOnce(first)
      .mockImplementation(async () => MemoryBoardBackend.create());
    const holder = new BoardRuntimeHolder(factory);
    await holder.get();
    await holder.reset();
    expect(first.closed).toBe(true);
    expect(await holder.get()).not.toBe(first);
  });

  it('baut im Server nach dem Speichern der Board-Konfig neu auf', async () => {
    const factory = vi.fn(async (): Promise<BoardBackend> => MemoryBoardBackend.create());
    const server = await TestServer.start({ boardBackend: factory });
    try {
      await server.app.boardRuntime.get();
      expect((await server.put('/api/board-config', INPUT)).status).toBe(200);
      await server.app.boardRuntime.get();
      expect(factory).toHaveBeenCalledTimes(2);
    } finally {
      await server.dispose();
    }
  });

  it('schließt im Server die alte Laufzeit nach dem Speichern der Board-Konfig', async () => {
    const { server, backend } = await TestServer.startWithBoard();
    try {
      await server.app.boardRuntime.get();
      expect((await server.put('/api/board-config', INPUT)).status).toBe(200);
      expect(backend.closed).toBe(true);
    } finally {
      await server.dispose();
    }
  });

  it('schließt im Server die aktuelle Laufzeit beim Beenden', async () => {
    const { server, backend } = await TestServer.startWithBoard();
    await server.app.boardRuntime.get();
    expect(backend.closed).toBe(false);
    await server.dispose();
    expect(backend.closed).toBe(true);
  });

  it('stuft SQLite-Datei- und Zugriffsfehler als Verbindungsfehler ein, Busy und Eindeutigkeit nicht', () => {
    expect(BoardRuntimeHolder.isConnectionError(sqliteError(14))).toBe(true);
    expect(BoardRuntimeHolder.isConnectionError(sqliteError(8))).toBe(true);
    expect(BoardRuntimeHolder.isConnectionError(sqliteError(26))).toBe(true);
    expect(BoardRuntimeHolder.isConnectionError(sqliteError(3))).toBe(true);
    expect(BoardRuntimeHolder.isConnectionError(sqliteError(1038))).toBe(true); // CANTOPEN_ISDIR = 14 | 4<<8
    expect(BoardRuntimeHolder.isConnectionError(Object.assign(new Error('database is not open'), { code: 'ERR_INVALID_STATE' }))).toBe(true);

    expect(BoardRuntimeHolder.isConnectionError(sqliteError(5))).toBe(false);
    expect(BoardRuntimeHolder.isConnectionError(sqliteError(517))).toBe(false);
    expect(BoardRuntimeHolder.isConnectionError(sqliteError(6))).toBe(false);
    expect(BoardRuntimeHolder.isConnectionError(sqliteError(2067))).toBe(false);
    expect(BoardRuntimeHolder.isConnectionError(sqliteError(10))).toBe(false);
    expect(BoardRuntimeHolder.isConnectionError(sqliteError(13))).toBe(false);
    expect(BoardRuntimeHolder.isConnectionError(new HarnessError('board-db-storage', 'voll'))).toBe(false);
  });

  it('verwirft die Laufzeit nach einem SQLite-Dateifehler', async () => {
    const first = await MemoryBoardBackend.create();
    const factory = vi.fn<() => Promise<BoardBackend>>()
      .mockResolvedValueOnce(first)
      .mockImplementation(async () => MemoryBoardBackend.create());
    const holder = new BoardRuntimeHolder(factory);
    await expect(holder.run(async () => { throw sqliteError(14); })).rejects.toMatchObject({ code: 'board-db-unreachable' });
    expect(first.closed).toBe(true);
    expect(await holder.get()).not.toBe(first);
  });

  it('meldet SQLITE_BUSY als conflict und baut nicht neu auf', async () => {
    const first = await MemoryBoardBackend.create();
    const factory = vi.fn<() => Promise<BoardBackend>>().mockResolvedValue(first);
    const holder = new BoardRuntimeHolder(factory);
    await expect(holder.run(async () => { throw sqliteError(5, 'database is locked'); })).rejects.toMatchObject({ code: 'conflict' });
    expect(first.closed).toBe(false);
    expect(factory).toHaveBeenCalledTimes(1);
  });

  it('meldet eine nicht öffenbare Board-Datei beim Aufbau als board-db-unreachable und merkt sich den Fehler nicht', async () => {
    const factory = vi.fn<() => Promise<BoardBackend>>()
      .mockRejectedValueOnce(sqliteError(14))
      .mockImplementation(async () => MemoryBoardBackend.create());
    const holder = new BoardRuntimeHolder(factory);
    await expect(holder.get()).rejects.toMatchObject({ code: 'board-db-unreachable', message: expect.stringContaining('Board-Datei') });
    await expect(holder.get()).resolves.toBeTruthy();
    expect(factory).toHaveBeenCalledTimes(2);
  });

  it('meldet einen nicht anlegbaren Ordner beim Aufbau als board-db-unreachable', async () => {
    const holder = new BoardRuntimeHolder(async () => {
      throw Object.assign(new Error("EACCES: permission denied, mkdir 'X:/x'"), { code: 'EACCES', syscall: 'mkdir' });
    });
    await expect(holder.get()).rejects.toMatchObject({ code: 'board-db-unreachable' });
  });

  it('lässt einen EACCES beim Öffnen unverändert durch', async () => {
    const error = Object.assign(new Error("EACCES: permission denied, open 'X:/x'"), { code: 'EACCES', syscall: 'open' });
    const holder = new BoardRuntimeHolder(async () => { throw error; });
    await expect(holder.get()).rejects.toBe(error);
  });

  it('reicht board-db-storage durch, ohne neu aufzubauen', async () => {
    const first = await MemoryBoardBackend.create();
    const factory = vi.fn<() => Promise<BoardBackend>>().mockResolvedValue(first);
    const holder = new BoardRuntimeHolder(factory);
    await expect(holder.run(async () => { throw new HarnessError('board-db-storage', 'x'); })).rejects.toMatchObject({ code: 'board-db-storage' });
    expect(first.closed).toBe(false);
    expect(factory).toHaveBeenCalledTimes(1);
  });
});
