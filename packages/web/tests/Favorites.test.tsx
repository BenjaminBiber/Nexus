// @vitest-environment jsdom
import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest';
import { cleanup, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { FavoriteDto, SessionDto } from '../src/api/types.js';
import { SessionsPage } from '../src/pages/SessionsPage.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { fakeApi } from './helpers/fakeApi.js';
import { renderWithApp } from './helpers/renderWithApp.js';

beforeEach(() => FakeEventSource.reset());
afterEach(cleanup);

const session: SessionDto = {
  id: 'aaaaaaaa-1', projectId: 'p1', tickets: ['T-0042'], ticketInfo: [{ id: 'T-0042', title: 'Login bauen', status: 'In Progress' }],
  cwd: 'D:/wt/T-0042', state: 'running', displayState: 'running', listening: true, startedAt: '2026-10-10T10:00:00.000Z',
  lastEventAt: new Date().toISOString(), endedAt: null, runtimeMs: 1000, title: null, model: null, subagents: [],
  client: 'claude-code', runName: null,
};

const favorite: FavoriteDto = {
  sessionId: 'aaaaaaaa-1', note: 'Weiter mit Tests', tickets: ['T-0042'], ticketInfo: session.ticketInfo, cwd: 'D:/wt/T-0042', cwdExists: true,
  title: null, markedAt: '2026-10-10T10:05:00.000Z', state: 'running', displayState: 'running', live: true,
  lastEventAt: session.lastEventAt, resumedAs: [], command: 'cd "D:/wt/T-0042" && claude --resume aaaaaaaa-1',
};

// Öffnet das Seitenfenster mit den Favoriten
async function openSheet() {
  await userEvent.click(await screen.findByRole('button', { name: /^Favoriten/ }));
  return screen.getByRole('dialog', { name: 'Favoriten' });
}

// Rendert die Seite mit austauschbaren Favoriten
function open(favorites: FavoriteDto[], overrides: Parameters<typeof fakeApi>[0] = {}) {
  const api = fakeApi({ getSessions: async () => [session], getFavorites: async () => favorites, ...overrides });
  renderWithApp(<SessionsPage />, { api, path: '/p/p1/sessions', route: '/p/:pid/sessions' });
}

describe('Favoriten in der Session-Übersicht', () => {
  it('zeigt den Stern mit Beschriftung und markiert eine Session', async () => {
    const markFavorite = vi.fn(async () => favorite);
    open([], { markFavorite });
    const star = await screen.findByRole('button', { name: 'Zu Favoriten hinzufügen' });
    expect(star.getAttribute('aria-pressed')).toBe('false');
    await userEvent.click(star);
    expect(markFavorite).toHaveBeenCalledWith('p1', 'aaaaaaaa-1', undefined);
  });

  it('zeigt einen Favoriten mit Notiz, Ticket, Verzeichnis und Befehl', async () => {
    open([favorite]);
    const panel = await openSheet();
    expect(within(panel).getByDisplayValue('Weiter mit Tests')).toBeTruthy();
    expect(within(panel).getByRole('link', { name: /T-0042 Login bauen/ })).toBeTruthy();
    expect(within(panel).getByText('D:/wt/T-0042')).toBeTruthy();
    expect(within(panel).getByText('cd "D:/wt/T-0042" && claude --resume aaaaaaaa-1')).toBeTruthy();
    const stars = screen.getAllByRole('button', { name: 'Aus Favoriten entfernen' });
    expect(stars.some((b) => b.getAttribute('aria-pressed') === 'true')).toBe(true);
  });

  it('speichert eine geänderte Notiz', async () => {
    const markFavorite = vi.fn(async () => favorite);
    open([favorite], { markFavorite });
    await openSheet();
    const input = await screen.findByDisplayValue('Weiter mit Tests');
    await userEvent.clear(input);
    await userEvent.type(input, 'Neu');
    await userEvent.click(screen.getByRole('button', { name: 'Notiz speichern' }));
    expect(markFavorite).toHaveBeenCalledWith('p1', 'aaaaaaaa-1', 'Neu');
    expect(await screen.findByText('Gespeichert')).toBeTruthy();
  });

  it('entfernt einen Favoriten', async () => {
    const unmarkFavorite = vi.fn(async () => undefined);
    open([favorite], { unmarkFavorite });
    await openSheet();
    await userEvent.click(await screen.findByRole('button', { name: 'Favorit entfernen' }));
    expect(unmarkFavorite).toHaveBeenCalledWith('p1', 'aaaaaaaa-1');
  });

  it('weist auf ein fehlendes Arbeitsverzeichnis hin und lässt den Befehl stehen', async () => {
    open([{ ...favorite, cwdExists: false, live: false }]);
    await openSheet();
    expect(await screen.findByText(/Arbeitsverzeichnis nicht mehr vorhanden/)).toBeTruthy();
    expect(screen.getByRole('button', { name: /Befehl kopieren/ })).toBeTruthy();
    expect(screen.getByText(/nicht mehr in der Übersicht/)).toBeTruthy();
  });

  it('zeigt fortgesetzte Sessions am Favoriten', async () => {
    open([{ ...favorite, resumedAs: ['bbbbbbbb-2'] }]);
    await openSheet();
    expect(await screen.findByText(/Fortgesetzt als bbbbbbbb/)).toBeTruthy();
  });

  it('erklärt im Leerzustand, wie man markiert', async () => {
    open([]);
    await openSheet();
    expect(await screen.findByText('Noch keine Favoriten')).toBeTruthy();
    expect(screen.getByText(/Markiere eine Session/)).toBeTruthy();
  });
});
