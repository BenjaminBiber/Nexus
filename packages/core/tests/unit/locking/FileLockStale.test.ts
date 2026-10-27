import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { mkdtempSync, rmSync, writeFileSync, existsSync, readFileSync, readdirSync, utimesSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

// Haken vor jedem rename der Sperrdatei: simuliert, was andere Prozesse genau in diesem Moment tun
const hook = vi.hoisted(() => ({ beforeRename: null as null | (() => Promise<void>), beforeLink: null as null | (() => Promise<void>) }));

vi.mock('node:fs/promises', async (importOriginal) => {
  const real = await importOriginal<typeof import('node:fs/promises')>();
  return {
    ...real,
    rename: async (from: string, to: string) => {
      const before = hook.beforeRename;
      hook.beforeRename = null;
      if (before) await before();
      return real.rename(from, to);
    },
    link: async (from: string, to: string) => {
      const before = hook.beforeLink;
      hook.beforeLink = null;
      if (before) await before();
      return real.link(from, to);
    },
  };
});

const { FileLock } = await import('../../../src/locking/FileLock.js');

// Zugriff auf das Entfernen einer verwaisten Sperre (sonst nur über acquire erreichbar)
type Remover = { removeStale(expected: string | null): Promise<void> };
const remover = (lock: InstanceType<typeof FileLock>) => lock as unknown as Remover;

describe('FileLock – verwaiste Sperre entfernen', () => {
  let dir: string;
  let lockPath: string;
  const write = (token: string, at = Date.now()) => writeFileSync(lockPath, JSON.stringify({ pid: process.pid, at, token }));
  const token = () => (JSON.parse(readFileSync(lockPath, 'utf8')) as { token: string }).token;
  const leftovers = () => readdirSync(dir).filter((f) => f.includes('.stale.'));

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'harness-lock-'));
    lockPath = join(dir, 'x.lock');
    hook.beforeRename = null;
    hook.beforeLink = null;
  });
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it('entfernt die Sperre, wenn das Kennzeichen noch das verwaiste ist', async () => {
    write('alt');
    await remover(new FileLock(lockPath)).removeStale('alt');
    expect(existsSync(lockPath)).toBe(false);
    expect(leftovers()).toEqual([]);
  });

  it('Kennzeichen geändert → die Sperre bleibt stehen, auch wenn sie sich kurz vor dem Umbenennen ändert', async () => {
    write('alt');
    // Der alte Halter gibt frei, ein anderer Prozess holt die Sperre neu – genau vor dem Umbenennen
    hook.beforeRename = async () => {
      rmSync(lockPath);
      write('neu');
    };
    await remover(new FileLock(lockPath)).removeStale('alt');
    expect(existsSync(lockPath)).toBe(true);
    expect(token()).toBe('neu');
    expect(leftovers()).toEqual([]);
  });

  it('legt eine schon neu vergebene Sperre gar nicht erst beiseite (keine Lücke für einen dritten Prozess)', async () => {
    write('neu');
    let renamed = false;
    hook.beforeRename = async () => {
      renamed = true;
    };
    await remover(new FileLock(lockPath)).removeStale('alt');
    expect(renamed).toBe(false);
    expect(token()).toBe('neu');
  });

  it('zwei Entferner gewinnen nicht beide: der zweite lässt die neu vergebene Sperre stehen', async () => {
    write('alt');
    const first = remover(new FileLock(lockPath));
    const second = remover(new FileLock(lockPath));
    // Beide haben „alt“ als verwaist erkannt. Während der erste umbenennen will, räumt der zweite auf
    // und ein dritter Prozess holt die Sperre neu.
    hook.beforeRename = async () => {
      await second.removeStale('alt');
      expect(existsSync(lockPath)).toBe(false);
      write('neu');
    };
    await first.removeStale('alt');
    expect(existsSync(lockPath)).toBe(true);
    expect(token()).toBe('neu');
    expect(leftovers()).toEqual([]);
  });

  it('gleichzeitige Entferner derselben verwaisten Sperre enden ohne Fehler und ohne Reste', async () => {
    write('alt');
    await Promise.all([remover(new FileLock(lockPath)).removeStale('alt'), remover(new FileLock(lockPath)).removeStale('alt')]);
    expect(existsSync(lockPath)).toBe(false);
    expect(leftovers()).toEqual([]);
  });

  it('stellt die Sperre wieder her, ohne eine inzwischen neu angelegte zu überschreiben', async () => {
    write('alt');
    // Vor dem Umbenennen wechselt das Kennzeichen; während die Sperre beiseite liegt, holt ein anderer sie neu
    hook.beforeRename = async () => write('zwischen');
    hook.beforeLink = async () => write('dritter');
    await remover(new FileLock(lockPath)).removeStale('alt');
    expect(token()).toBe('dritter');
    expect(leftovers()).toEqual([]);
  });

  it('unlesbare Sperre: nur entfernen, wenn sie älter als staleMs ist', async () => {
    writeFileSync(lockPath, '');
    await remover(new FileLock(lockPath, { staleMs: 60000 })).removeStale(null);
    expect(existsSync(lockPath)).toBe(true);
    expect(leftovers()).toEqual([]);

    const old = new Date(Date.now() - 120000);
    utimesSync(lockPath, old, old);
    await remover(new FileLock(lockPath, { staleMs: 60000 })).removeStale(null);
    expect(existsSync(lockPath)).toBe(false);
    expect(leftovers()).toEqual([]);
  });
});
