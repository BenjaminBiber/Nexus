// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, useLocation } from 'react-router-dom';
import { useBoards, useColorScheme, useConnection, useNav, useNavigate, useSettings, useUsage } from '@nexus/theme-sdk';
import { App } from '../src/App.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import { registerTheme, unregisterTheme } from '../src/theme/registry.js';
import type { ConnectionState } from '../src/live/liveContext.js';
import type { UsageOverviewDto } from '../src/api/usageTypes.js';
import { fakeApi } from './helpers/fakeApi.js';

// Zustand der Live-Verbindung je Test vorgeben; der Rest des Moduls bleibt echt
let liveState: ConnectionState = 'connected';
vi.mock('../src/live/useLive.js', async (importOriginal) => {
  const real = await importOriginal<typeof import('../src/live/useLive.js')>();
  return { ...real, useLiveConnection: () => ({ ...real.useLiveConnection(), state: liveState }) };
});

afterEach(() => {
  liveState = 'connected';
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
  const connection = useConnection();
  const navigate = useNavigate();
  const { pathname } = useLocation();
  return (
    <div>
      <ul aria-label="nav">{nav.map((n) => <li key={n.id}>{`${n.label}|${n.href}|${n.active ? 'aktiv' : ''}`}</li>)}</ul>
      <p>{`board:${boards.current?.project}/${boards.current?.board}`}</p>
      <p>{`usage:${usage.enabled}:${usage.fiveHour?.percent ?? '-'}`}</p>
      <ul aria-label="accounts">
        {usage.accounts.map((a) => (
          <li key={a.id}>{`${a.id}|${a.name}|${a.fiveHour?.percent ?? '-'}|${a.sevenDay?.percent ?? '-'}|${a.limitFiveHour ?? '-'}|${a.limitWeek ?? '-'}|${a.reason ?? '-'}`}</li>
        ))}
      </ul>
      <p>{`vorgabe:${usage.defaultLimit}`}</p>
      <p>{`verbunden:${connection.connected}`}</p>
      <p>{`settings:${settings.values?.theme ?? 'lädt'}`}</p>
      <p>{`scheme:${scheme.scheme}/${scheme.themeId}`}</p>
      <p>{`pfad:${pathname}`}</p>
      <button onClick={() => navigate('/p/p1/docs')}>zu Docs</button>
    </div>
  );
}

// Überblick mit zwei aktiven Accounts: einer mit Werten und eigener 5-Stunden-Grenze, einer ohne Werte mit Wochengrenze
const overview: UsageOverviewDto = {
  enabled: true,
  defaultLimit: 90,
  accounts: [
    {
      id: 'a',
      name: 'Privat',
      fallback: false,
      limitFiveHour: 80,
      limitWeek: null,
      usage: { available: true, reason: null, plan: null, fiveHour: { percent: 42, resetsAt: null }, sevenDay: { percent: 18, resetsAt: null }, fetchedAt: null },
    },
    {
      id: 'b',
      name: 'Arbeit',
      fallback: true,
      limitFiveHour: null,
      limitWeek: 70,
      usage: { available: false, reason: 'token-expired', plan: null, fiveHour: null, sevenDay: null, fetchedAt: null },
    },
  ],
};

describe('NexusThemeHost', () => {
  it('liefert einer Theme-Shell Navigation, Board, Verbrauch, Einstellungen und Farbschema und wechselt die Route', async () => {
    registerTheme({ id: 'test-host', slots: { Shell: HookShell } });
    document.documentElement.dataset.theme = 'test-host';
    const base = fakeApi();
    const api = fakeApi({
      getUserSettings: async () => ({ ...(await base.getUserSettings()), theme: 'test-host' as never }),
      getUsageOverview: async () => overview,
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
    expect(await screen.findByText('usage:true:42')).toBeTruthy();
    expect(await screen.findByText('a|Privat|42|18|80|-|-')).toBeTruthy();
    expect(screen.getByText('b|Arbeit|-|-|-|70|token-expired')).toBeTruthy();
    expect(screen.getByText('vorgabe:90')).toBeTruthy();
    expect(screen.getByText('verbunden:true')).toBeTruthy();
    expect(await screen.findByText('settings:test-host')).toBeTruthy();
    expect(screen.getByText('scheme:light/test-host')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'zu Docs' }));
    await waitFor(() => expect(screen.getByText('pfad:/p/p1/docs')).toBeTruthy());
    expect(screen.getByText('Docs|/p/p1/docs|aktiv')).toBeTruthy();
  });

  it.each([
    ['disconnected', 'false'],
    ['connecting', 'false'],
    ['connected', 'true'],
  ] as const)('meldet über useConnection den Zustand %s der Live-Verbindung', async (state, erwartet) => {
    liveState = state;
    registerTheme({ id: 'test-host', slots: { Shell: HookShell } });
    document.documentElement.dataset.theme = 'test-host';
    const base = fakeApi();
    const api = fakeApi({ getUserSettings: async () => ({ ...(await base.getUserSettings()), theme: 'test-host' as never }) });
    render(
      <ApiProvider api={api}>
        <MemoryRouter initialEntries={['/p/p1/sessions']}>
          <App />
        </MemoryRouter>
      </ApiProvider>,
    );
    expect(await screen.findByText(`verbunden:${erwartet}`)).toBeTruthy();
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
});
