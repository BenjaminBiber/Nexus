// @vitest-environment jsdom
import { describe, it, expect, afterEach, beforeEach, vi } from 'vitest';
import { act, cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { App } from '../src/App.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ApiError } from '../src/api/ApiError.js';
import type { SessionDto } from '../src/api/types.js';
import { SessionsPage } from '../src/pages/SessionsPage.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';
import { fakeApi } from './helpers/fakeApi.js';
import { renderWithApp } from './helpers/renderWithApp.js';

beforeEach(() => FakeEventSource.reset());
afterEach(cleanup);

// Baut eine Session; nicht genannte Felder haben Standardwerte
function session(id: string, extra: Partial<SessionDto> = {}): SessionDto {
  return {
    id, projectId: 'p1', tickets: [], ticketInfo: [], cwd: 'D:/wt/T-0042', state: 'running', displayState: 'running', listening: true,
    startedAt: '2026-10-10T10:00:00.000Z', lastEventAt: new Date().toISOString(), endedAt: null, runtimeMs: 125000,
    title: null, model: null, subagents: [], client: 'claude-code', runName: null, ...extra,
  };
}

const t42 = { id: 'T-0042', title: 'Login bauen', status: 'In Progress' as const };
const t43 = { id: 'T-0043', title: 'Token prüfen', status: 'To Do' as const };

const list = [
  session('aaaaaaaa-1', {
    tickets: ['T-0042'], ticketInfo: [t42],
    subagents: [
      { id: 'a1', type: 'Explore', state: 'running', startedAt: '2026-10-10T10:01:00.000Z', endedAt: null },
      { id: 'a2', type: 'general-purpose', state: 'ended', startedAt: '2026-10-10T10:00:10.000Z', endedAt: '2026-10-10T10:00:40.000Z' },
    ],
  }),
  session('bbbbbbbb-2', { tickets: ['T-0043'], ticketInfo: [t43], state: 'waiting', displayState: 'waiting', runtimeMs: 3_900_000 }),
  session('cccccccc-3', { state: 'idle', displayState: 'idle', cwd: 'D:/anderswo' }),
  session('dddddddd-4', { tickets: ['T-0042'], ticketInfo: [t42], state: 'ended', displayState: 'ended', listening: false, endedAt: '2026-10-10T10:30:00.000Z' }),
  session('eeeeeeee-5', { tickets: ['T-0043'], ticketInfo: [t43], state: 'running', displayState: 'stale', listening: false }),
];

// Rendert die Seite mit austauschbarem Abruf; mit showAll wird der Filter „alle“ gewählt
async function open(overrides: Parameters<typeof fakeApi>[0] = {}, showAll = true) {
  const getSessions = vi.fn(async () => list);
  const api = fakeApi({ getSessions, ...overrides });
  renderWithApp(<SessionsPage />, { api, path: '/p/p1/sessions', route: '/p/:pid/sessions' });
  if (showAll) await userEvent.click(await screen.findByRole('button', { name: 'alle' }));
  return { getSessions };
}

describe('SessionsPage', () => {
  it('gruppiert nach Ticket mit Nummer und Titel und führt „Nicht zugeordnet“ gesondert', async () => {
    await open();
    const g42 = await screen.findByRole('region', { name: 'T-0042 Login bauen' });
    const g43 = screen.getByRole('region', { name: 'T-0043 Token prüfen' });
    const none = screen.getByRole('region', { name: 'Nicht zugeordnet' });
    expect(within(g42).getByText('aaaaaaaa')).toBeTruthy();
    expect(within(g43).getByText('bbbbbbbb')).toBeTruthy();
    expect(within(none).getByText('cccccccc')).toBeTruthy();
    expect(within(none).getByText(/harness board claim/)).toBeTruthy();
    expect(within(g42).getByRole('link', { name: 'Ticket öffnen' }).getAttribute('href')).toBe('/p/p1/board/t/T-0042');
  });

  it('kopiert die volle Session-ID per Knopf', async () => {
    const writeText = vi.fn(async () => undefined);
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    await open();
    const g42 = await screen.findByRole('region', { name: 'T-0042 Login bauen' });
    await userEvent.click(within(g42).getByRole('button', { name: 'Session-ID kopieren: aaaaaaaa-1' }));
    expect(writeText).toHaveBeenCalledWith('aaaaaaaa-1');
    expect(await within(g42).findByText('Kopiert')).toBeTruthy();
  });

  it('beschriftet jeden Zustand mit Text', async () => {
    await open();
    await screen.findByRole('region', { name: 'T-0042 Login bauen' });
    const states = screen.getAllByTestId('session-state').map((e) => e.textContent);
    expect(states.sort()).toEqual(['beendet', 'läuft', 'untätig', 'veraltet', 'wartet auf Eingabe']);
  });

  it('zeigt nur laufende Subagenten als Baum und blendet beendete aus', async () => {
    await open();
    const tree = await screen.findByRole('list', { name: 'Subagenten von aaaaaaaa' });
    const items = within(tree).getAllByRole('listitem');
    expect(items).toHaveLength(1);
    expect(items[0].textContent).toContain('Explore');
    expect(items[0].textContent).toContain('läuft');
    expect(screen.queryByText('general-purpose')).toBeNull();
  });

  it('sortiert Sessions je Ticket nach Zustand: Aktives zuerst, Beendetes zuletzt', async () => {
    await open();
    const g42 = await screen.findByRole('region', { name: 'T-0042 Login bauen' });
    const ids = within(g42).getAllByText(/^[a-d]{8}$/).map((e) => e.textContent);
    expect(ids).toEqual(['aaaaaaaa', 'dddddddd']);
    const g43 = screen.getByRole('region', { name: 'T-0043 Token prüfen' });
    const ids43 = within(g43).getAllByText(/^[b-e]{8}$/).map((e) => e.textContent);
    expect(ids43).toEqual(['bbbbbbbb', 'eeeeeeee']);
  });

  it('zeigt Laufzeit und letzte Aktivität', async () => {
    await open();
    await screen.findByRole('region', { name: 'T-0042 Login bauen' });
    expect(screen.getAllByText('2 Min.').length).toBeGreaterThan(0);
    expect(screen.getByText('1 Std. 05 Min.')).toBeTruthy();
    expect(screen.getAllByText(/vor weniger als einer Minute/).length).toBeGreaterThan(0);
  });

  it('hebt veraltete und beendete Sessions optisch ab (Klasse) und zeigt das Arbeitsverzeichnis', async () => {
    await open();
    await screen.findByRole('region', { name: 'T-0042 Login bauen' });
    const stale = screen.getByText('eeeeeeee').closest('article')!;
    expect(stale.className).toContain('stale');
    const ended = screen.getByText('dddddddd').closest('article')!;
    expect(ended.className).toContain('ended');
    expect(screen.getByText('D:/anderswo')).toBeTruthy();
  });

  it('zeigt standardmäßig nur laufende und untätige Sessions', async () => {
    await open({}, false);
    await screen.findByRole('region', { name: 'T-0042 Login bauen' });
    expect(screen.getByText('aaaaaaaa')).toBeTruthy();
    expect(screen.getByText('cccccccc')).toBeTruthy();
    expect(screen.queryByText('bbbbbbbb')).toBeNull();
    expect(screen.queryByText('dddddddd')).toBeNull();
    expect(screen.queryByText('eeeeeeee')).toBeNull();
    const group = screen.getByRole('group', { name: 'Filter' });
    expect(within(group).getByRole('button', { name: 'läuft' }).getAttribute('aria-pressed')).toBe('true');
    expect(within(group).getByRole('button', { name: 'untätig' }).getAttribute('aria-pressed')).toBe('true');
  });

  it('filtert nach Zustand (Mehrfachauswahl)', async () => {
    await open({}, false);
    await screen.findByRole('region', { name: 'T-0042 Login bauen' });
    const group = screen.getByRole('group', { name: 'Filter' });
    // Zustände abwählen bzw. hinzunehmen: nur „wartet auf Eingabe“
    await userEvent.click(within(group).getByRole('button', { name: 'läuft' }));
    await userEvent.click(within(group).getByRole('button', { name: 'untätig' }));
    await userEvent.click(within(group).getByRole('button', { name: 'wartet auf Eingabe' }));
    expect(screen.getByText('bbbbbbbb')).toBeTruthy();
    expect(screen.queryByText('aaaaaaaa')).toBeNull();
    expect(screen.queryByRole('region', { name: 'Nicht zugeordnet' })).toBeNull();
    // „alle“ plus fünf Zustände
    expect(within(group).getAllByRole('button')).toHaveLength(6);
  });

  it('zeigt Leerzustand mit Hinweisen auf harness init und das Plugin', async () => {
    await open({ getSessions: async () => [] }, false);
    expect(await screen.findByText('Keine Sessions')).toBeTruthy();
    expect(screen.getByText('Starte ein Ticket, dann erscheint die Session hier.')).toBeTruthy();
    expect(screen.getByText('Noch keine Hook-Meldungen empfangen')).toBeTruthy();
    expect(screen.getByText('harness init')).toBeTruthy();
  });

  it('zeigt Lade- und Fehlerzustand', async () => {
    await open({ getSessions: async () => { throw new ApiError('x', 'Server nicht erreichbar', 0); } }, false);
    expect(screen.getByText('Sessions werden geladen …')).toBeTruthy();
    expect(await screen.findByText('Server nicht erreichbar')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Erneut versuchen' })).toBeTruthy();
  });

  it('aktualisiert sich bei einer Live-Meldung ohne Neuladen', async () => {
    const getSessions = vi.fn(async () => list);
    const api = fakeApi({ getSessions });
    render(
      <ApiProvider api={api}>
        <MemoryRouter initialEntries={['/p/p1/sessions']}>
          <App />
        </MemoryRouter>
      </ApiProvider>,
    );
    await screen.findByRole('region', { name: 'T-0042 Login bauen' });
    const before = getSessions.mock.calls.length;
    act(() => FakeEventSource.current().emit({ type: 'sessions' }));
    await vi.waitFor(() => expect(getSessions.mock.calls.length).toBeGreaterThan(before));
  });
});
