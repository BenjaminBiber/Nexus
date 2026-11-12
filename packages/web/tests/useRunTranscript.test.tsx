// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import type { ApiClient } from '../src/api/ApiClient.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { DispatchRunShotDto } from '../src/api/dispatchTypes.js';
import type { TranscriptEntry } from '../src/api/sessionDetailTypes.js';
import { TRANSCRIPT_PAGE_SIZE } from '../src/hooks/useSessionDetail.js';
import { useRunTranscript } from '../src/hooks/useRunTranscript.js';
import { fakeApi, transcriptPage } from './helpers/fakeApi.js';

afterEach(cleanup);

const NAME = 'ARC-0001-20261016-101500.log';
const AT = '2026-10-16T08:16:00.000Z';
const text = (id: string): TranscriptEntry => ({ kind: 'text', id, at: AT, text: `Text ${id}` });
const tool = (id: string, done: boolean): TranscriptEntry => ({
  kind: 'tool', id, at: AT, name: 'Read', summary: 'a.ts', input: null,
  result: done ? { text: 'ok', isError: false, truncated: false } : null,
});
const SHOT: DispatchRunShotDto = { file: '001.png', toolUseId: 'toolu_1', at: AT, mediaType: 'image/png', bytes: 12 };

// Startet den Hook mit einer Test-Attrappe als API
function start(api: ApiClient, running = false, name = NAME) {
  const wrapper = ({ children }: { children: ReactNode }) => <ApiProvider api={api}>{children}</ApiProvider>;
  return renderHook((props: { name: string; running: boolean }) => useRunTranscript(props.name, props.running, 20), {
    wrapper,
    initialProps: { name, running },
  });
}

describe('useRunTranscript', () => {
  it('lädt Verlauf und Screenshots eines beendeten Laufs genau einmal', async () => {
    const getDispatchRunTranscript = vi.fn(async () => transcriptPage([text('a'), tool('b', true)]));
    const getDispatchRunShots = vi.fn(async () => [SHOT]);
    const { result } = start(fakeApi({ getDispatchRunTranscript, getDispatchRunShots }));
    expect(result.current).toEqual({ status: 'loading', available: false, entries: [], shots: [] });
    await waitFor(() => expect(result.current.status).toBe('ready'));
    expect(result.current.available).toBe(true);
    expect(result.current.entries.map((entry) => entry.id)).toEqual(['a', 'b']);
    expect(result.current.shots).toEqual([SHOT]);
    expect(getDispatchRunTranscript).toHaveBeenCalledTimes(1);
    expect(getDispatchRunTranscript).toHaveBeenCalledWith(NAME, null, TRANSCRIPT_PAGE_SIZE);
    await new Promise((done) => setTimeout(done, 80));
    expect(getDispatchRunTranscript).toHaveBeenCalledTimes(1);
    expect(getDispatchRunShots).toHaveBeenCalledTimes(1);
  });

  it('holt weitere Seiten, solange eine Seite voll ist', async () => {
    const full = Array.from({ length: TRANSCRIPT_PAGE_SIZE }, (_, index) => text(`e${index}`));
    const getDispatchRunTranscript = vi.fn(async (_name: string, after: string | null) =>
      after === null ? transcriptPage(full, { cursor: 'c1' }) : transcriptPage([text('letzter')], { cursor: 'c2' }));
    const { result } = start(fakeApi({ getDispatchRunTranscript }));
    await waitFor(() => expect(result.current.status).toBe('ready'));
    expect(result.current.entries).toHaveLength(TRANSCRIPT_PAGE_SIZE + 1);
    expect(getDispatchRunTranscript.mock.calls.map((call) => call[1])).toEqual([null, 'c1']);
  });

  it('meldet einen Lauf ohne Transcript als nicht verfügbar', async () => {
    const { result } = start(fakeApi({ getDispatchRunShots: async () => [SHOT] }));
    await waitFor(() => expect(result.current.status).toBe('ready'));
    expect(result.current).toEqual({ status: 'ready', available: false, entries: [], shots: [SHOT] });
  });

  it('bleibt bei Fehlern nicht hängen', async () => {
    const api = fakeApi({
      getDispatchRunTranscript: async () => { throw new Error('Server weg'); },
      getDispatchRunShots: async () => { throw new Error('Server weg'); },
    });
    const { result } = start(api);
    await waitFor(() => expect(result.current.status).toBe('ready'));
    expect(result.current).toEqual({ status: 'ready', available: false, entries: [], shots: [] });
  });

  it('lädt bei einem laufenden Lauf ab dem Cursor nach und nach dem Ende noch genau einmal', async () => {
    let later = false;
    const getDispatchRunTranscript = vi.fn(async (_name: string, after: string | null) => {
      if (after === null) return transcriptPage([tool('a', false)], { cursor: 'c1' });
      return later ? transcriptPage([tool('a', true), text('b')], { cursor: 'c2' }) : transcriptPage([], { cursor: after });
    });
    const getDispatchRunShots = vi.fn(async () => (later ? [SHOT] : []));
    const { result, rerender } = start(fakeApi({ getDispatchRunTranscript, getDispatchRunShots }), true);
    await waitFor(() => expect(result.current.entries).toHaveLength(1));
    expect(result.current.shots).toEqual([]);

    later = true;
    await waitFor(() => expect(result.current.entries).toHaveLength(2));
    // Der Aufruf steht an seiner Stelle, jetzt mit Ergebnis; nachgeladen wurde ab dem Cursor
    expect(result.current.entries[0]).toEqual(tool('a', true));
    expect(result.current.entries[1]!.id).toBe('b');
    expect(getDispatchRunTranscript.mock.calls[1]![1]).toBe('c1');
    await waitFor(() => expect(result.current.shots).toEqual([SHOT]));

    const before = getDispatchRunTranscript.mock.calls.length;
    rerender({ name: NAME, running: false });
    await waitFor(() => expect(getDispatchRunTranscript).toHaveBeenCalledTimes(before + 1));
    await new Promise((done) => setTimeout(done, 100));
    expect(getDispatchRunTranscript).toHaveBeenCalledTimes(before + 1);
  });

  it('beginnt bei einem anderen Lauf von vorn und verwirft späte Antworten zum alten', async () => {
    let release: () => void = () => undefined;
    const getDispatchRunTranscript = vi.fn(async (name: string) => {
      if (name === NAME) {
        await new Promise<void>((done) => { release = done; });
        return transcriptPage([text('alt')]);
      }
      return transcriptPage([text('neu')]);
    });
    const { result, rerender } = start(fakeApi({ getDispatchRunTranscript }));
    rerender({ name: 'ARC-0002-20261016-111500.log', running: false });
    await waitFor(() => expect(result.current.entries.map((entry) => entry.id)).toEqual(['neu']));
    release();
    await new Promise((done) => setTimeout(done, 30));
    expect(result.current.entries.map((entry) => entry.id)).toEqual(['neu']);
  });

  it('beendet den Zeitgeber beim Aushängen', async () => {
    const getDispatchRunTranscript = vi.fn(async () => transcriptPage([text('a')]));
    const { result, unmount } = start(fakeApi({ getDispatchRunTranscript }), true);
    await waitFor(() => expect(result.current.status).toBe('ready'));
    unmount();
    const calls = getDispatchRunTranscript.mock.calls.length;
    await new Promise((done) => setTimeout(done, 100));
    expect(getDispatchRunTranscript).toHaveBeenCalledTimes(calls);
  });
});
