import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { HarnessError } from '@harness/core';
import { SqlErrorMapper } from '../src/SqlErrorMapper.js';

// Fehler, wie ihn node:sqlite wirft: code 'ERR_SQLITE_ERROR' plus erweiterter Ergebniscode
const sqliteError = (errcode: number, errstr = 'x') => Object.assign(new Error(errstr), { code: 'ERR_SQLITE_ERROR', errcode, errstr });

// Führt fn aus und liefert den geworfenen Fehler
const errorOf = (fn: () => void): unknown => {
  try {
    fn();
  } catch (e) {
    return e;
  }
  throw new Error('kein Fehler');
};

describe('SqlErrorMapper', () => {
  it('macht aus einem Deadlock-Opfer (1205) einen conflict', () => {
    const mapped = SqlErrorMapper.map(Object.assign(new Error('deadlock'), { number: 1205 }));
    expect(mapped).toBeInstanceOf(HarnessError);
    expect(mapped).toMatchObject({ code: 'conflict', message: 'Gleichzeitige Änderung – bitte erneut versuchen.' });
  });

  it('erkennt die Nummer auch am ursprünglichen Treiberfehler', () => {
    const mapped = SqlErrorMapper.map(Object.assign(new Error('x'), { originalError: { info: { number: 1205 } } }));
    expect(mapped).toMatchObject({ code: 'conflict' });
  });

  it('macht aus Speichermangel (701) einen board-db-overloaded', () => {
    const mapped = SqlErrorMapper.map(Object.assign(new Error('insufficient system memory'), { number: 701 }));
    expect(mapped).toBeInstanceOf(HarnessError);
    expect(mapped).toMatchObject({ code: 'board-db-overloaded', message: expect.stringContaining('Arbeitsspeicher') });
  });

  it('lässt andere Fehler unverändert', () => {
    const error = Object.assign(new Error('pk'), { number: 2627 });
    expect(SqlErrorMapper.map(error)).toBe(error);
  });

  it('macht aus einer gesperrten SQLite-Datei (BUSY/LOCKED) einen conflict', () => {
    for (const errcode of [5, 6, 517, 262]) {
      const mapped = SqlErrorMapper.map(sqliteError(errcode, 'database is locked'));
      expect(mapped, String(errcode)).toBeInstanceOf(HarnessError);
      expect(mapped, String(errcode)).toMatchObject({ code: 'conflict', message: 'Gleichzeitige Änderung – bitte erneut versuchen.' });
    }
  });

  it('macht aus einem vollen Datenträger (SQLITE_FULL) einen board-db-storage', () => {
    const mapped = SqlErrorMapper.map(sqliteError(13, 'database or disk is full'));
    expect(mapped).toBeInstanceOf(HarnessError);
    expect(mapped).toMatchObject({ code: 'board-db-storage', message: expect.stringContaining('Datenträger ist voll') });
  });

  it('macht aus einem E/A-Fehler (SQLITE_IOERR) einen board-db-storage mit Kurztext', () => {
    for (const errcode of [10, 3338]) {
      const mapped = SqlErrorMapper.map(sqliteError(errcode, 'disk I/O error'));
      expect(mapped, String(errcode)).toBeInstanceOf(HarnessError);
      expect(mapped, String(errcode)).toMatchObject({ code: 'board-db-storage', message: expect.stringContaining('disk I/O error') });
    }
  });

  it('lässt doppelte SQLite-Schlüssel unverändert durch', () => {
    for (const errcode of [2067, 1555]) {
      const error = sqliteError(errcode, 'constraint failed');
      expect(SqlErrorMapper.map(error), String(errcode)).toBe(error);
      expect(SqlErrorMapper.isUniqueViolation(error), String(errcode)).toBe(true);
    }
  });

  it('erkennt doppelte Schlüssel von SQL Server und SQLite', () => {
    expect(SqlErrorMapper.isUniqueViolation(Object.assign(new Error('pk'), { number: 2627 }))).toBe(true);
    expect(SqlErrorMapper.isUniqueViolation(Object.assign(new Error('ix'), { number: 2601 }))).toBe(true);
    expect(SqlErrorMapper.isUniqueViolation(Object.assign(new Error('x'), { originalError: { info: { number: 2627 } } }))).toBe(true);
    expect(SqlErrorMapper.isUniqueViolation(sqliteError(1299, 'constraint failed'))).toBe(false);
    expect(SqlErrorMapper.isUniqueViolation(sqliteError(787, 'constraint failed'))).toBe(false);
    expect(SqlErrorMapper.isUniqueViolation(Object.assign(new Error('deadlock'), { number: 1205 }))).toBe(false);
    expect(SqlErrorMapper.isUniqueViolation(new Error('x'))).toBe(false);
    expect(SqlErrorMapper.isUniqueViolation(null)).toBe(false);
  });

  it('lässt einen errcode ohne ERR_SQLITE_ERROR unverändert', () => {
    const error = Object.assign(new Error('x'), { errcode: 5 });
    expect(SqlErrorMapper.map(error)).toBe(error);
  });

  it('erkennt echte node:sqlite-Fehler', async () => {
    const { DatabaseSync } = await import('node:sqlite');
    const dir = mkdtempSync(join(tmpdir(), 'harness-sqlerr-'));
    const file = join(dir, 't.db');
    const a = new DatabaseSync(file);
    let b: InstanceType<typeof DatabaseSync> | undefined;
    try {
      a.exec('PRAGMA journal_mode=WAL');
      a.exec('CREATE TABLE T (Id INTEGER PRIMARY KEY, K TEXT UNIQUE)');
      a.exec("INSERT INTO T VALUES (1, 'a')");
      expect(SqlErrorMapper.isUniqueViolation(errorOf(() => a.exec("INSERT INTO T VALUES (2, 'a')")))).toBe(true);
      expect(SqlErrorMapper.isUniqueViolation(errorOf(() => a.exec("INSERT INTO T VALUES (1, 'b')")))).toBe(true);
      b = new DatabaseSync(file, { timeout: 0 });
      a.exec('BEGIN IMMEDIATE');
      expect(SqlErrorMapper.map(errorOf(() => b!.exec('BEGIN IMMEDIATE')))).toMatchObject({ code: 'conflict' });
      a.exec('ROLLBACK');
    } finally {
      a.close();
      b?.close();
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
