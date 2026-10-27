// @vitest-environment jsdom
import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { ApiProvider } from '../src/api/ApiContext.js';
import { SessionNotifier } from '../src/components/SessionNotifier.js';
import { LiveProvider } from '../src/live/LiveProvider.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { fakeApi } from './helpers/fakeApi.js';

// Browser-Benachrichtigung zum Mitschreiben; die Erlaubnis gilt als erteilt
class FakeNotification {
  static permission = 'granted';
  static shown: FakeNotification[] = [];
  onclick: ((event: { preventDefault: () => void }) => void) | null = null;
  closed = false;
  constructor(readonly title: string, readonly options: NotificationOptions) {
    FakeNotification.shown.push(this);
  }
  close() {
    this.closed = true;
  }
}

beforeEach(() => {
  FakeEventSource.reset();
  FakeNotification.shown = [];
  vi.stubGlobal('Notification', FakeNotification);
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

function openTab() {
  const openEvents = vi.fn((..._args: unknown[]) => new FakeEventSource());
  const api = fakeApi({ getSessions: vi.fn(async () => []), openEvents });
  render(
    <ApiProvider api={api}>
      <MemoryRouter initialEntries={['/p/p1/sessions']}>
        <LiveProvider pid="p1">
          <SessionNotifier pid="p1" />
          <Routes>
            <Route path="*" element={<span>Seite</span>} />
            <Route path="/p/p1/board/t/:id" element={<span>Ticket-Panel</span>} />
          </Routes>
        </LiveProvider>
      </MemoryRouter>
    </ApiProvider>,
  );
  return { openEvents };
}

describe('Meldungen des Servers im Tab', () => {
  it('meldet den Tab mit erlaubten Benachrichtigungen beim Server an', () => {
    const { openEvents } = openTab();
    expect(openEvents).toHaveBeenCalledWith('p1', null, null, true);
  });

  it('zeigt die Meldung und navigiert beim Klick im selben Tab', async () => {
    openTab();
    const focus = vi.spyOn(window, 'focus').mockImplementation(() => undefined);
    act(() =>
      FakeEventSource.current().emit({ type: 'notice', kind: 'wartet', tag: 'attention', title: 'Session braucht deine Antwort', body: 'webshop', url: '/p/p1/board/t/T-1' }),
    );
    expect(FakeNotification.shown).toHaveLength(1);
    expect(FakeNotification.shown[0].options).toMatchObject({ body: 'webshop', tag: 'attention' });
    act(() => FakeNotification.shown[0].onclick?.({ preventDefault: () => undefined }));
    expect(focus).toHaveBeenCalled();
    expect(await screen.findByText('Ticket-Panel')).toBeTruthy();
    expect(FakeNotification.shown[0].closed).toBe(true);
  });
});
