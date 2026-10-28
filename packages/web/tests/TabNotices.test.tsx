// @vitest-environment jsdom
import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest';
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { ApiProvider } from '../src/api/ApiContext.js';
import { SessionNotifier } from '../src/components/SessionNotifier.js';
import { LiveProvider } from '../src/live/LiveProvider.js';
import type { Platform } from '../src/platform/Platform.js';
import { PlatformProvider } from '../src/platform/PlatformContext.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { fakeApi } from './helpers/fakeApi.js';
import { fakePlatform } from './helpers/fakePlatform.js';

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

// Fenster der Desktop-App: die Erlaubnis und die Meldung kommen von der Plattform
function openWindow(platform: Platform) {
  const openEvents = vi.fn((..._args: unknown[]) => new FakeEventSource());
  const api = fakeApi({ getSessions: vi.fn(async () => []), openEvents });
  render(
    <ApiProvider api={api}>
      <PlatformProvider value={platform}>
        <MemoryRouter initialEntries={['/p/p1/sessions']}>
          <LiveProvider pid="p1">
            <SessionNotifier pid="p1" />
          </LiveProvider>
        </MemoryRouter>
      </PlatformProvider>
    </ApiProvider>,
  );
  return { openEvents };
}

describe('Meldungen des Servers in der Desktop-App', () => {
  // Das Fenster der App kennt die Meldung des Browsers nicht; allein die Plattform weiß, ob gemeldet werden darf
  beforeEach(() => vi.stubGlobal('Notification', undefined));

  it('meldet das Fenster mit notify=1 an, sobald das System Meldungen erlaubt', async () => {
    const { openEvents } = openWindow(fakePlatform({ notificationPermission: vi.fn(async () => 'granted' as const) }));
    await waitFor(() => expect(openEvents).toHaveBeenLastCalledWith('p1', null, null, true));
  });

  it('meldet das Fenster ohne Erlaubnis ohne notify=1 an, damit der Server selbst meldet', async () => {
    const platform = fakePlatform({ notificationPermission: vi.fn(async () => 'denied' as const) });
    const { openEvents } = openWindow(platform);
    await waitFor(() => expect(platform.notificationPermission).toHaveBeenCalled());
    expect(openEvents).toHaveBeenCalledWith('p1', null);
    expect(openEvents).not.toHaveBeenCalledWith('p1', null, null, true);
  });

  it('gibt die Meldung des Servers an die Plattform weiter', async () => {
    const notify = vi.fn(async () => undefined);
    const { openEvents } = openWindow(fakePlatform({ notificationPermission: vi.fn(async () => 'granted' as const), notify }));
    await waitFor(() => expect(openEvents).toHaveBeenLastCalledWith('p1', null, null, true));
    act(() =>
      FakeEventSource.current().emit({ type: 'notice', kind: 'wartet', tag: 'attention', title: 'Session braucht deine Antwort', body: 'webshop', url: '/p/p1/board/t/T-1' }),
    );
    expect(notify).toHaveBeenCalledWith(
      expect.objectContaining({ title: 'Session braucht deine Antwort', body: 'webshop', tag: 'attention', url: '/p/p1/board/t/T-1' }),
    );
    expect(FakeNotification.shown).toHaveLength(0);
  });
});
