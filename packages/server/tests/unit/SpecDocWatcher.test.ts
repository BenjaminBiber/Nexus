import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SuperpowersDocScanner } from '@harness/core';
import { SpecDocWatcher } from '../../src/events/SpecDocWatcher.js';

describe('SpecDocWatcher', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('meldet eine neue Datei in docs/superpowers/specs genau einmal', async () => {
    const base = mkdtempSync(join(tmpdir(), 'harness-docs-'));
    const specs = join(base, 'docs', 'superpowers', 'specs');
    mkdirSync(specs, { recursive: true });
    execFileSync('git', ['init', '-q'], { cwd: base, windowsHide: true });
    const scanner = new SuperpowersDocScanner();
    const watcher = new SpecDocWatcher(async () => scanner.signature(base), 10000);
    const seen: unknown[] = [];
    watcher.watch('p1', (e) => seen.push(e));
    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(10000);
    expect(seen).toEqual([]);
    writeFileSync(join(specs, '2026-10-22-neu-design.md'), '# Neu\n');
    await vi.advanceTimersByTimeAsync(10000);
    expect(seen).toEqual([{ type: 'docs-changed', projectId: 'p1' }]);
    await vi.advanceTimersByTimeAsync(10000);
    expect(seen).toHaveLength(1);
    watcher.close();
    rmSync(base, { recursive: true, force: true });
  });

  it('meldet ohne Änderung der Signatur nichts', async () => {
    const watcher = new SpecDocWatcher(async () => 'gleich', 10000);
    const seen: unknown[] = [];
    watcher.watch('p1', (e) => seen.push(e));
    await vi.advanceTimersByTimeAsync(30000);
    expect(seen).toEqual([]);
    watcher.close();
  });

  it('fragt ohne Zuhörer nicht ab und hält nach der Abmeldung an', async () => {
    const signatureOf = vi.fn(async (_pid: string) => 'a');
    const watcher = new SpecDocWatcher(signatureOf, 10000);
    await vi.advanceTimersByTimeAsync(30000);
    expect(signatureOf).not.toHaveBeenCalled();
    const stop = watcher.watch('p1', () => undefined);
    await vi.advanceTimersByTimeAsync(10000);
    expect(signatureOf).toHaveBeenCalledWith('p1');
    stop();
    signatureOf.mockClear();
    await vi.advanceTimersByTimeAsync(30000);
    expect(signatureOf).not.toHaveBeenCalled();
    watcher.close();
  });

  it('meldet allen Empfängern desselben Projekts, anderen Projekten nicht', async () => {
    const signatures = new Map([['p1', 'a'], ['p2', 'x']]);
    const watcher = new SpecDocWatcher(async (pid) => signatures.get(pid)!, 10000);
    const one: unknown[] = [];
    const two: unknown[] = [];
    const other: unknown[] = [];
    watcher.watch('p1', (e) => one.push(e));
    watcher.watch('p1', (e) => two.push(e));
    watcher.watch('p2', (e) => other.push(e));
    await vi.advanceTimersByTimeAsync(0);
    signatures.set('p1', 'b');
    await vi.advanceTimersByTimeAsync(10000);
    expect(one).toEqual([{ type: 'docs-changed', projectId: 'p1' }]);
    expect(two).toEqual([{ type: 'docs-changed', projectId: 'p1' }]);
    expect(other).toEqual([]);
    watcher.close();
  });

  it('übersteht einen Fehler beim Abfragen und meldet danach weiter', async () => {
    let signature = 'a';
    let fail = false;
    const watcher = new SpecDocWatcher(async () => {
      if (fail) throw new Error('kaputt');
      return signature;
    }, 10000);
    const seen: unknown[] = [];
    watcher.watch('p1', (e) => seen.push(e));
    await vi.advanceTimersByTimeAsync(0);
    fail = true;
    await vi.advanceTimersByTimeAsync(10000);
    expect(seen).toEqual([]);
    fail = false;
    signature = 'b';
    await vi.advanceTimersByTimeAsync(10000);
    expect(seen).toEqual([{ type: 'docs-changed', projectId: 'p1' }]);
    watcher.close();
  });

  it('fragt im Takt von 10 s ab', async () => {
    const signatureOf = vi.fn(async (_pid: string) => 'a');
    const watcher = new SpecDocWatcher(signatureOf, 10000);
    watcher.watch('p1', () => undefined);
    await vi.advanceTimersByTimeAsync(0);
    signatureOf.mockClear();
    await vi.advanceTimersByTimeAsync(9000);
    expect(signatureOf).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1000);
    expect(signatureOf).toHaveBeenCalledTimes(1);
    watcher.close();
  });
});
