// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, useLocation } from 'react-router-dom';
import { useBoards, useColorScheme, useNav, useNavigate, useSettings, useUsage } from '@nexus/theme-sdk';
import { App } from '../src/App.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import { registerTheme, unregisterTheme } from '../src/theme/registry.js';
import { dbQuestion, dbQuestionList, fakeApi, inboxItem, inboxList } from './helpers/fakeApi.js';

afterEach(() => {
  cleanup();
  unregisterTheme('test-host');
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.themeMode;
});

// Shell eines Themes, die alles nur über die Hooks des SDK bezieht
function HookShell() {
  const nav = useNav();
  const boards = useBoards();
  const usage = useUsage();
  const settings = useSettings();
  const scheme = useColorScheme();
  const navigate = useNavigate();
  const { pathname } = useLocation();
  return (
    <div>
      <ul aria-label="nav">{nav.map((n) => <li key={n.id}>{`${n.label}|${n.href}|${n.active ? 'aktiv' : ''}`}</li>)}</ul>
      <p>{`board:${boards.current?.project}/${boards.current?.board}`}</p>
      <p>{`usage:${usage.enabled}:${usage.fiveHour?.percent ?? '-'}`}</p>
      <p>{`settings:${settings.values?.theme ?? 'lädt'}`}</p>
      <p>{`scheme:${scheme.scheme}/${scheme.themeId}`}</p>
      <p>{`pfad:${pathname}`}</p>
      <button onClick={() => navigate('/p/p1/docs')}>zu Docs</button>
    </div>
  );
}

describe('NexusThemeHost', () => {
  it('liefert einer Theme-Shell Navigation, Board, Verbrauch, Einstellungen und Farbschema und wechselt die Route', async () => {
    registerTheme({ id: 'test-host', slots: { Shell: HookShell } });
    document.documentElement.dataset.theme = 'test-host';
    const usage = { available: true, reason: null, plan: null, fiveHour: { percent: 37, resetsAt: null }, sevenDay: null, fetchedAt: null };
    const base = fakeApi();
    const api = fakeApi({
      getUserSettings: async () => ({ ...(await base.getUserSettings()), theme: 'test-host' as never }),
      getUsageOverview: async () => ({ enabled: true, defaultLimit: 90, accounts: [{ id: 'a', name: 'A', fallback: false, limitFiveHour: null, limitWeek: null, usage }] }),
    });
    render(
      <ApiProvider api={api}>
        <MemoryRouter initialEntries={['/p/p1/sessions']}>
          <App />
        </MemoryRouter>
      </ApiProvider>,
    );
    expect(await screen.findByText('Sessions|/p/p1/sessions|aktiv')).toBeTruthy();
    expect(screen.getByText('Board|/p/p1/board|')).toBeTruthy();
    expect(screen.getByText('Einstellungen|/p/p1/settings|')).toBeTruthy();
    expect(await screen.findByText('board:ARC/1')).toBeTruthy();
    expect(await screen.findByText('usage:true:37')).toBeTruthy();
    expect(await screen.findByText('settings:test-host')).toBeTruthy();
    expect(screen.getByText('scheme:light/test-host')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'zu Docs' }));
    await waitFor(() => expect(screen.getByText('pfad:/p/p1/docs')).toBeTruthy());
    expect(screen.getByText('Docs|/p/p1/docs|aktiv')).toBeTruthy();
  });

  it('wählt über useBoards wie die Seitenleiste ein anderes Board', async () => {
    function Chooser() {
      const boards = useBoards();
      return <button onClick={() => boards.choose({ project: 'OTH', board: 3 })}>{`wählen:${boards.current?.board}`}</button>;
    }
    registerTheme({ id: 'test-host', slots: { Shell: Chooser } });
    document.documentElement.dataset.theme = 'test-host';
    const activateDbBoard = vi.fn(async () => {});
    render(
      <ApiProvider api={fakeApi({ activateDbBoard: activateDbBoard as never, getUserSettings: async () => ({ ...(await fakeApi().getUserSettings()), theme: 'test-host' as never }) })}>
        <MemoryRouter initialEntries={['/p/p1/sessions']}>
          <App />
        </MemoryRouter>
      </ApiProvider>,
    );
    await userEvent.click(await screen.findByRole('button', { name: 'wählen:1' }));
    await waitFor(() => expect(activateDbBoard).toHaveBeenCalledWith(3));
  });

  it('liefert wie die Seitenleiste Zähler für Meine Aufgaben, eigene offene Fragen und neue Docs', async () => {
    function BadgeShell() {
      const nav = useNav();
      return <ul aria-label="nav">{nav.map((n) => <li key={n.id}>{`${n.id}:${n.badge ?? '-'}`}</li>)}</ul>;
    }
    registerTheme({ id: 'test-host', slots: { Shell: BadgeShell } });
    document.documentElement.dataset.theme = 'test-host';
    const doc = (path: string) => ({ path, title: path, group: 'spec' as const, kind: 'spec' as const, topic: null, date: null, mtimeMs: 1, exists: true });
    const base = fakeApi();
    const api = fakeApi({
      getUserSettings: async () => ({ ...(await base.getUserSettings()), theme: 'test-host' as never }),
      getInbox: async () => inboxList([inboxItem('ticket:ARC-0001', 'merge', 'Eins'), inboxItem('note:3', 'note', 'Zwei')]),
      getDbQuestions: async () => dbQuestionList([dbQuestion('ARC-F0001', 'Eins'), dbQuestion('ARC-F0002', 'Fremd', { mine: false })]),
      getDocs: async () => ({ ...(await base.getDocs('p1')), docs: [doc('a.md'), doc('b.md'), doc('c.md')] }),
    });
    render(
      <ApiProvider api={api}>
        <MemoryRouter initialEntries={['/p/p1/sessions']}>
          <App />
        </MemoryRouter>
      </ApiProvider>,
    );
    expect(await screen.findByText('inbox:2')).toBeTruthy();
    expect(await screen.findByText('questions:1')).toBeTruthy();
    expect(await screen.findByText('docs:3')).toBeTruthy();
    expect(screen.getByText('board:-')).toBeTruthy();
  });

  it('lässt den Zähler ohne offene Einträge weg', async () => {
    function BadgeShell() {
      const nav = useNav();
      return <ul aria-label="nav">{nav.map((n) => <li key={n.id}>{`${n.id}:${n.badge ?? '-'}`}</li>)}</ul>;
    }
    registerTheme({ id: 'test-host', slots: { Shell: BadgeShell } });
    document.documentElement.dataset.theme = 'test-host';
    const base = fakeApi();
    render(
      <ApiProvider api={fakeApi({ getUserSettings: async () => ({ ...(await base.getUserSettings()), theme: 'test-host' as never }) })}>
        <MemoryRouter initialEntries={['/p/p1/sessions']}>
          <App />
        </MemoryRouter>
      </ApiProvider>,
    );
    expect(await screen.findByText('inbox:-')).toBeTruthy();
    expect(screen.getByText('questions:-')).toBeTruthy();
    expect(screen.getByText('docs:-')).toBeTruthy();
  });
});
