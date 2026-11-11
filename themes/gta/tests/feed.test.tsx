// @vitest-environment jsdom
// Board-Stand laden und aktuell halten: Ereignisstrom, kein Stau, Sicherheitsnetz, Aufräumen, verspätete Antworten
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useBoardFeed } from '../src/data/feed.js';
import type { BoardView } from '../src/data/types.js';

// Falsche EventSource: merkt sich alle Ströme, Nachrichten kommen per emit()
class FakeEventSource {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSED = 2;
  static all: FakeEventSource[] = [];
  readyState = FakeEventSource.OPEN;
  onmessage: ((event: { data: string }) => void) | null = null;
  onerror: (() => void) | null = null;
  constructor(readonly url: string) {
    FakeEventSource.all.push(this);
  }
  close() {
    this.readyState = FakeEventSource.CLOSED;
  }
  emit(message: unknown) {
    act(() => this.onmessage?.({ data: typeof message === 'string' ? message : JSON.stringify(message) }));
  }
}

// Falsches fetch: jede Anfrage wartet, bis der Test sie beantwortet
interface Call {
  url: string;
  signal: AbortSignal;
  answer(json: unknown): void;
  fail(): void;
}
let calls: Call[];

function fakeFetch(url: string, init: { signal: AbortSignal }) {
  return new Promise((resolve, reject) => {
    calls.push({
      url,
      signal: init.signal,
      answer: (json) => resolve({ ok: true, status: 200, json: async () => json }),
      fail: () => reject(new TypeError('offline')),
    });
  });
}

const flush = () =>
  act(async () => {
    for (let i = 0; i < 20; i++) await Promise.resolve();
  });

const view = (boardId: number, title = 'Warenkorb'): BoardView => ({
  boardId,
  columns: [
    {
      id: 'in_progress',
      title: 'In Arbeit',
      tickets: [{ id: 'NOVA-0001', title, column: 'in_progress', blockedReason: null, stepsDone: 0, stepsTotal: 2, priority: 'normal', assignee: null }],
    },
  ],
});

async function answer(call: Call, json: unknown) {
  call.answer(json);
  await flush();
}

const tick = (ms: number) => act(() => vi.advanceTimersByTime(ms));
const last = <T,>(list: T[]) => list[list.length - 1]!;

beforeEach(() => {
  calls = [];
  FakeEventSource.all = [];
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'setInterval', 'clearInterval'] });
  vi.stubGlobal('fetch', vi.fn(fakeFetch));
  vi.stubGlobal('EventSource', FakeEventSource);
  vi.spyOn(console, 'error');
  vi.spyOn(console, 'warn');
  vi.spyOn(console, 'log');
});

afterEach(() => {
  // Nichts in die Konsole
  expect(console.error).not.toHaveBeenCalled();
  expect(console.warn).not.toHaveBeenCalled();
  expect(console.log).not.toHaveBeenCalled();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe('useBoardFeed', () => {
  it('ohne Board: kein Stand, keine Anfrage, kein Strom', () => {
    const { result } = renderHook(() => useBoardFeed('p1', null));
    expect(result.current).toEqual({ view: null, failed: false });
    act(() => vi.advanceTimersByTime(60_000));
    expect(calls).toHaveLength(0);
    expect(FakeEventSource.all).toHaveLength(0);
  });

  it('lädt sofort und öffnet den Ereignisstrom des Boards', async () => {
    const { result } = renderHook(() => useBoardFeed('p1', 3));
    expect(calls.map((c) => c.url)).toEqual(['/api/db/boards/3/view']);
    expect(FakeEventSource.all.map((s) => s.url)).toEqual(['/api/projects/p1/events?dbBoard=3']);
    expect(result.current.view).toBeNull();
    await answer(calls[0]!, view(3));
    expect(result.current).toEqual({ view: view(3), failed: false });
  });

  it('lädt bei dbboard für dieses Board und bei reset neu, alles andere nicht', async () => {
    const { result } = renderHook(() => useBoardFeed('p1', 3));
    await answer(calls[0]!, view(3));
    const source = FakeEventSource.all[0]!;
    source.emit({ type: 'dbboard', boardId: 4 });
    source.emit({ type: 'hello' });
    source.emit({ type: 'dbquestions', projectId: 3 });
    source.emit('kein json');
    expect(calls).toHaveLength(1);
    source.emit({ type: 'dbboard', boardId: 3 });
    expect(calls).toHaveLength(2);
    await answer(calls[1]!, view(3, 'Kasse'));
    expect(result.current.view?.columns[0]!.tickets[0]!.title).toBe('Kasse');
    source.emit({ type: 'reset' });
    expect(calls).toHaveLength(3);
  });

  it('kein Stau: während einer Anfrage wird danach genau einmal nachgeladen', async () => {
    renderHook(() => useBoardFeed('p1', 3));
    const source = FakeEventSource.all[0]!;
    for (let i = 0; i < 4; i++) source.emit({ type: 'dbboard', boardId: 3 });
    expect(calls).toHaveLength(1);
    await answer(calls[0]!, view(3));
    expect(calls).toHaveLength(2);
    await answer(calls[1]!, view(3));
    expect(calls).toHaveLength(2);
  });

  it('Sicherheitsnetz: alle 30 s neu laden, wenn der Strom nicht offen ist', async () => {
    renderHook(() => useBoardFeed('p1', 3));
    await answer(calls[0]!, view(3));
    tick(30_000);
    expect(calls).toHaveLength(1);
    FakeEventSource.all[0]!.readyState = FakeEventSource.CONNECTING;
    tick(29_999);
    expect(calls).toHaveLength(1);
    tick(1);
    expect(calls).toHaveLength(2);
  });

  it('Sicherheitsnetz ohne EventSource: lädt alle 30 s', async () => {
    vi.stubGlobal('EventSource', undefined);
    renderHook(() => useBoardFeed('p1', 3));
    await answer(calls[0]!, view(3));
    tick(30_000);
    expect(calls).toHaveLength(2);
    await answer(calls[1]!, view(3));
    tick(30_000);
    expect(calls).toHaveLength(3);
  });

  it('beim Abbauen: Strom zu, Anfrage abgebrochen, keine Timer mehr', () => {
    const { unmount } = renderHook(() => useBoardFeed('p1', 3));
    unmount();
    expect(FakeEventSource.all[0]!.readyState).toBe(FakeEventSource.CLOSED);
    expect(calls[0]!.signal.aborted).toBe(true);
    tick(120_000);
    expect(calls).toHaveLength(1);
    expect(vi.getTimerCount()).toBe(0);
  });

  it('beim Wechsel des Boards: Stand sofort null, Altes aufgeräumt, verspätete Antwort zählt nicht', async () => {
    const { result, rerender } = renderHook(({ id }) => useBoardFeed('p1', id), { initialProps: { id: 3 } });
    await answer(calls[0]!, view(3));
    FakeEventSource.all[0]!.emit({ type: 'dbboard', boardId: 3 });
    const old = calls[1]!;
    rerender({ id: 4 });
    expect(result.current).toEqual({ view: null, failed: false });
    expect(FakeEventSource.all[0]!.readyState).toBe(FakeEventSource.CLOSED);
    expect(old.signal.aborted).toBe(true);
    expect(last(calls).url).toBe('/api/db/boards/4/view');
    expect(last(FakeEventSource.all).url).toBe('/api/projects/p1/events?dbBoard=4');
    await answer(old, view(3, 'verspätet'));
    expect(result.current.view).toBeNull();
    await answer(last(calls), view(4));
    expect(result.current.view?.boardId).toBe(4);
    rerender({ id: 4 });
    expect(calls).toHaveLength(3);
  });

  it('Fehler: failed, der letzte gute Stand bleibt; danach wieder gut', async () => {
    const { result } = renderHook(() => useBoardFeed('p1', 3));
    await answer(calls[0]!, view(3));
    const source = FakeEventSource.all[0]!;
    source.emit({ type: 'reset' });
    calls[1]!.fail();
    await flush();
    expect(result.current).toEqual({ view: view(3), failed: true });
    source.emit({ type: 'reset' });
    await answer(calls[2]!, { kaputt: true });
    expect(result.current).toEqual({ view: view(3), failed: true });
    source.emit({ type: 'reset' });
    await answer(calls[3]!, view(3, 'Kasse'));
    expect(result.current.failed).toBe(false);
    expect(result.current.view?.columns[0]!.tickets[0]!.title).toBe('Kasse');
  });

  it('Fehler beim ersten Laden: kein Stand, failed', async () => {
    const { result } = renderHook(() => useBoardFeed('p1', 3));
    calls[0]!.fail();
    await flush();
    expect(result.current).toEqual({ view: null, failed: true });
  });
});
