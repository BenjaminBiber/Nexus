import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { BoardChangeWatcher } from '../../src/events/BoardChangeWatcher.js';

describe('BoardChangeWatcher', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('meldet eine Änderung des Kennwerts genau einmal an alle Empfänger dieses Boards', async () => {
    const markers = new Map([[1, 'a'], [2, 'x']]);
    const watcher = new BoardChangeWatcher(async (id) => markers.get(id)!, 2500);
    const one: unknown[] = [];
    const two: unknown[] = [];
    const other: unknown[] = [];
    watcher.watch(1, (e) => one.push(e));
    watcher.watch(1, (e) => two.push(e));
    watcher.watch(2, (e) => other.push(e));
    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(2500);
    expect(one).toEqual([]);
    markers.set(1, 'b');
    await vi.advanceTimersByTimeAsync(2500);
    expect(one).toEqual([{ type: 'dbboard', boardId: 1 }]);
    expect(two).toEqual([{ type: 'dbboard', boardId: 1 }]);
    expect(other).toEqual([]);
    await vi.advanceTimersByTimeAsync(2500);
    expect(one).toHaveLength(1);
    watcher.close();
  });

  it('fragt nur Boards mit Empfängern ab und hält ohne Empfänger an', async () => {
    const markerOf = vi.fn(async (_id: number) => 'a');
    const watcher = new BoardChangeWatcher(markerOf, 2500);
    await vi.advanceTimersByTimeAsync(10000);
    expect(markerOf).not.toHaveBeenCalled();
    const stop = watcher.watch(7, () => undefined);
    await vi.advanceTimersByTimeAsync(2500);
    expect(markerOf).toHaveBeenCalledWith(7);
    stop();
    markerOf.mockClear();
    await vi.advanceTimersByTimeAsync(10000);
    expect(markerOf).not.toHaveBeenCalled();
  });

  it('übersteht Fehler beim Abfragen und meldet danach Änderungen', async () => {
    let fail = true;
    let marker = 'a';
    const watcher = new BoardChangeWatcher(async () => {
      if (fail) throw new Error('Datenbank weg');
      return marker;
    }, 2500);
    const events: unknown[] = [];
    watcher.watch(1, (e) => events.push(e));
    await vi.advanceTimersByTimeAsync(2500);
    fail = false;
    await vi.advanceTimersByTimeAsync(2500);
    expect(events).toEqual([]);
    marker = 'b';
    await vi.advanceTimersByTimeAsync(2500);
    expect(events).toEqual([{ type: 'dbboard', boardId: 1 }]);
    watcher.close();
  });

  it('meldet mit eigener Ereignisfunktion, z. B. dbquestions je Projekt', async () => {
    let marker = 'a';
    const watcher = new BoardChangeWatcher(async () => marker, 2500, (projectId) => ({ type: 'dbquestions' as const, projectId }));
    const events: unknown[] = [];
    watcher.watch(7, (e) => events.push(e));
    await vi.advanceTimersByTimeAsync(0);
    marker = 'b';
    await vi.advanceTimersByTimeAsync(2500);
    expect(events).toEqual([{ type: 'dbquestions', projectId: 7 }]);
    watcher.close();
  });
});
