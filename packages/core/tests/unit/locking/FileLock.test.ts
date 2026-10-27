import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdtempSync, rmSync, writeFileSync, existsSync, utimesSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { FileLock } from '../../../src/locking/FileLock.js';
import { HarnessError } from '../../../src/errors/HarnessError.js';

describe('FileLock', () => {
  let dir: string;
  let lockPath: string;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'harness-lock-'));
    lockPath = join(dir, 'x.lock');
  });
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it('legt die Sperre an und gibt sie nach der Aktion wieder frei', async () => {
    const lock = new FileLock(lockPath);
    const result = await lock.withLock(() => {
      expect(existsSync(lockPath)).toBe(true);
      return 42;
    });
    expect(result).toBe(42);
    expect(existsSync(lockPath)).toBe(false);
  });

  it('gibt die Sperre auch bei einem Fehler in der Aktion frei', async () => {
    const lock = new FileLock(lockPath);
    await expect(lock.withLock(() => { throw new Error('kaputt'); })).rejects.toThrow('kaputt');
    expect(existsSync(lockPath)).toBe(false);
  });

  it('wartet, bis eine fremde Sperre freigegeben wird', async () => {
    writeFileSync(lockPath, JSON.stringify({ pid: process.pid, at: Date.now(), token: 'fremd' }));
    setTimeout(() => rmSync(lockPath), 150);
    const lock = new FileLock(lockPath, { timeoutMs: 3000 });
    await lock.withLock(() => undefined);
    expect(existsSync(lockPath)).toBe(false);
  });

  it('räumt eine Sperre eines beendeten Prozesses auf', async () => {
    writeFileSync(lockPath, JSON.stringify({ pid: 2147483646, at: Date.now(), token: 'tot' }));
    const lock = new FileLock(lockPath, { timeoutMs: 3000 });
    await lock.withLock(() => undefined);
    expect(existsSync(lockPath)).toBe(false);
  });

  it('räumt eine zu alte Sperre auf, auch wenn der Prozess noch läuft', async () => {
    writeFileSync(lockPath, JSON.stringify({ pid: process.pid, at: Date.now() - 60000, token: 'alt' }));
    const old = new Date(Date.now() - 60000);
    utimesSync(lockPath, old, old);
    const lock = new FileLock(lockPath, { timeoutMs: 3000, staleMs: 500 });
    await lock.withLock(() => undefined);
    expect(existsSync(lockPath)).toBe(false);
  });

  it('wirft nach der Wartezeit einen HarnessError', async () => {
    writeFileSync(lockPath, JSON.stringify({ pid: process.pid, at: Date.now(), token: 'frisch' }));
    const lock = new FileLock(lockPath, { timeoutMs: 300, staleMs: 60000 });
    await expect(lock.withLock(() => undefined)).rejects.toThrow(HarnessError);
    await expect(lock.withLock(() => undefined)).rejects.toThrow(/Sperre/);
  });

  it('serialisiert gleichzeitige Aktionen im selben Prozess', async () => {
    const lock = new FileLock(lockPath, { timeoutMs: 5000 });
    let running = 0;
    let maxRunning = 0;
    const action = async () => {
      running++;
      maxRunning = Math.max(maxRunning, running);
      await new Promise((r) => setTimeout(r, 10));
      running--;
    };
    await Promise.all(Array.from({ length: 8 }, () => lock.withLock(action)));
    expect(maxRunning).toBe(1);
  });
});
