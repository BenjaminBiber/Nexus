// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render } from '@testing-library/react';
import { ApiProvider } from '../src/api/ApiContext.js';
import { LiveProvider } from '../src/live/LiveProvider.js';
import { useDbBoardWatch, useLiveHandler } from '../src/live/useLive.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { fakeApi } from './helpers/fakeApi.js';

beforeEach(() => FakeEventSource.reset());
afterEach(cleanup);

// Beobachtet ein DB-Board und reicht dessen Meldungen weiter
function Watcher({ boardId, onDbBoard }: { boardId: number | null; onDbBoard: (id: number) => void }) {
  useDbBoardWatch(boardId);
  useLiveHandler({ onDbBoard });
  return null;
}

describe('Live-Meldungen des DB-Boards', () => {
  it('öffnet den Strom mit dem beobachteten Board und meldet dbboard', () => {
    const openEvents = vi.fn((_pid: string, _board?: number | null) => new FakeEventSource());
    const onDbBoard = vi.fn();
    render(
      <ApiProvider api={fakeApi({ openEvents })}>
        <LiveProvider pid="p1">
          <Watcher boardId={7} onDbBoard={onDbBoard} />
        </LiveProvider>
      </ApiProvider>,
    );
    expect(openEvents).toHaveBeenLastCalledWith('p1', 7);
    act(() => FakeEventSource.current().emit({ type: 'dbboard', boardId: 7 }));
    expect(onDbBoard).toHaveBeenCalledWith(7);
  });

  it('verbindet beim Wechsel des Boards neu und schließt den alten Strom', () => {
    const openEvents = vi.fn((_pid: string, _board?: number | null) => new FakeEventSource());
    const api = fakeApi({ openEvents });
    const view = (boardId: number) => (
      <ApiProvider api={api}>
        <LiveProvider pid="p1">
          <Watcher boardId={boardId} onDbBoard={() => undefined} />
        </LiveProvider>
      </ApiProvider>
    );
    const { rerender } = render(view(7));
    const first = FakeEventSource.current();
    rerender(view(8));
    expect(first.closed).toBe(true);
    expect(openEvents).toHaveBeenLastCalledWith('p1', 8);
  });
});
