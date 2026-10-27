// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, waitFor } from '@testing-library/react';
import { ApiProvider } from '../src/api/ApiContext.js';
import { LiveProvider } from '../src/live/LiveProvider.js';
import { useDbProjectWatch, useLiveHandler } from '../src/live/useLive.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { fakeApi } from './helpers/fakeApi.js';

beforeEach(() => FakeEventSource.reset());
afterEach(cleanup);

// Beobachtet die Fragen eines Board-Projekts und reicht deren Meldungen weiter
function Watcher({ projectId, onDbQuestions }: { projectId: number | null; onDbQuestions: (id: number) => void }) {
  useDbProjectWatch(projectId);
  useLiveHandler({ onDbQuestions });
  return null;
}

describe('Live-Meldungen der Fragen auf der Board-Datenbank', () => {
  it('öffnet den Strom mit dem beobachteten Projekt und meldet dbquestions', () => {
    const openEvents = vi.fn((_pid: string, _board?: number | null, _project?: number | null) => new FakeEventSource());
    const onDbQuestions = vi.fn();
    render(
      <ApiProvider api={fakeApi({ openEvents })}>
        <LiveProvider pid="p1">
          <Watcher projectId={3} onDbQuestions={onDbQuestions} />
        </LiveProvider>
      </ApiProvider>,
    );
    expect(openEvents).toHaveBeenLastCalledWith('p1', null, 3);
    act(() => FakeEventSource.current().emit({ type: 'dbquestions', projectId: 3 }));
    expect(onDbQuestions).toHaveBeenCalledWith(3);
  });

  it('ruft ohne beobachtetes Projekt openEvents wie bisher mit zwei Argumenten auf', () => {
    const openEvents = vi.fn((_pid: string, _board?: number | null, _project?: number | null) => new FakeEventSource());
    render(
      <ApiProvider api={fakeApi({ openEvents })}>
        <LiveProvider pid="p1">
          <Watcher projectId={null} onDbQuestions={() => undefined} />
        </LiveProvider>
      </ApiProvider>,
    );
    expect(openEvents).toHaveBeenLastCalledWith('p1', null);
  });

  // Zwei Beobachter, die sich einzeln ausblenden lassen
  function Two({ first, second, a = 1, b = 2 }: { first: boolean; second: boolean; a?: number; b?: number }) {
    return (
      <>
        {first && <Watcher projectId={a} onDbQuestions={() => undefined} />}
        {second && <Watcher projectId={b} onDbQuestions={() => undefined} />}
      </>
    );
  }

  it('behält beim Ausblenden eines Beobachters das Projekt des anderen', async () => {
    const openEvents = vi.fn((_pid: string, _board?: number | null, _project?: number | null) => new FakeEventSource());
    const api = fakeApi({ openEvents });
    const tree = (first: boolean, second: boolean) => (
      <ApiProvider api={api}>
        <LiveProvider pid="p1" releaseDelayMs={20}>
          <Two first={first} second={second} />
        </LiveProvider>
      </ApiProvider>
    );
    const { rerender } = render(tree(true, true));
    expect(openEvents).toHaveBeenLastCalledWith('p1', null, 2);
    rerender(tree(true, false));
    expect(openEvents).toHaveBeenLastCalledWith('p1', null, 1);
    rerender(tree(false, false));
    // Ohne Beobachter erst nach der Verzögerung ohne Projekt
    expect(openEvents).toHaveBeenLastCalledWith('p1', null, 1);
    await waitFor(() => expect(openEvents).toHaveBeenLastCalledWith('p1', null));
  });

  it('behält beim Ausblenden des ersten Beobachters das Projekt des zweiten', () => {
    const openEvents = vi.fn((_pid: string, _board?: number | null, _project?: number | null) => new FakeEventSource());
    const api = fakeApi({ openEvents });
    const tree = (first: boolean) => (
      <ApiProvider api={api}>
        <LiveProvider pid="p1">
          <Two first={first} second />
        </LiveProvider>
      </ApiProvider>
    );
    const { rerender } = render(tree(true));
    rerender(tree(false));
    expect(openEvents).toHaveBeenLastCalledWith('p1', null, 2);
  });

  it('verbindet nicht neu, wenn ein Beobachter dasselbe Projekt kurz nach dem anderen übernimmt', async () => {
    const openEvents = vi.fn((_pid: string, _board?: number | null, _project?: number | null) => new FakeEventSource());
    const api = fakeApi({ openEvents });
    const tree = (first: boolean, second: boolean) => (
      <ApiProvider api={api}>
        <LiveProvider pid="p1" releaseDelayMs={50}>
          <Two first={first} second={second} a={1} b={1} />
        </LiveProvider>
      </ApiProvider>
    );
    const { rerender } = render(tree(true, false));
    const calls = openEvents.mock.calls.length;
    rerender(tree(false, false));
    rerender(tree(false, true));
    await new Promise((resolve) => setTimeout(resolve, 80));
    expect(openEvents.mock.calls.length).toBe(calls);
    expect(openEvents).toHaveBeenLastCalledWith('p1', null, 1);
  });
});
