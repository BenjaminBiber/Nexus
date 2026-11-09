// @vitest-environment jsdom
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { ThemeHostProvider, useBoards, useColorScheme, useNav, useNavigate, useOpenSearch, useSettings, useUsage } from '@nexus/theme-sdk';
import type { ThemeHost } from '@nexus/theme-sdk';

afterEach(cleanup);

// Host mit festen Werten und beobachtbaren Aktionen
function fakeHost(): ThemeHost {
  return {
    nav: [
      { id: 'board', label: 'Board', href: '/p/p1/board', active: true },
      { id: 'inbox', label: 'Meine Aufgaben', href: '/p/p1/inbox', active: false, badge: 2 },
    ],
    boards: {
      current: { project: 'ARC', board: 1 },
      projects: [{ id: 'ARC', name: 'Arches' }],
      boards: [{ id: 1, project: 'ARC', name: 'Sprint' }],
      loading: false,
      choose: vi.fn(),
    },
    usage: { enabled: true, available: true, reason: null, fiveHour: { percent: 42, resetsAt: null }, sevenDay: null },
    settings: { values: { theme: 'dark', claudeUsage: true, compactCards: false, heroImages: true }, loading: false, update: vi.fn(async () => {}) },
    colorScheme: { scheme: 'dark', themeId: 'dark', mode: 'dark' },
    navigate: vi.fn(),
    openSearch: vi.fn(),
    parts: { search: null, boardSwitcher: null, usage: null, navItems: {}, outlet: null, connection: null, status: null },
  };
}

function render_(host: ThemeHost, ui: React.ReactNode) {
  return render(<ThemeHostProvider value={host}>{ui}</ThemeHostProvider>);
}

describe('Hooks des SDK', () => {
  it('useNav liefert die Einträge der Navigation', () => {
    function Nav() {
      return <ul>{useNav().map((n) => <li key={n.id}>{`${n.label}|${n.href}|${n.active}|${n.badge ?? ''}`}</li>)}</ul>;
    }
    render_(fakeHost(), <Nav />);
    expect(screen.getByText('Board|/p/p1/board|true|')).toBeTruthy();
    expect(screen.getByText('Meine Aufgaben|/p/p1/inbox|false|2')).toBeTruthy();
  });

  it('useBoards liefert das aktuelle Board und wählt ein anderes', () => {
    const host = fakeHost();
    function Boards() {
      const boards = useBoards();
      return <button onClick={() => boards.choose({ project: 'ARC', board: 1 })}>{`${boards.current?.project}/${boards.boards[0]?.name}`}</button>;
    }
    render_(host, <Boards />);
    fireEvent.click(screen.getByRole('button', { name: 'ARC/Sprint' }));
    expect(host.boards.choose).toHaveBeenCalledWith({ project: 'ARC', board: 1 });
  });

  it('useUsage liefert den Verbrauch', () => {
    function Usage() {
      const usage = useUsage();
      return <span>{`${usage.enabled}:${usage.fiveHour?.percent}`}</span>;
    }
    render_(fakeHost(), <Usage />);
    expect(screen.getByText('true:42')).toBeTruthy();
  });

  it('useSettings liefert die Einstellungen und ändert sie', () => {
    const host = fakeHost();
    function Settings() {
      const settings = useSettings();
      return <button onClick={() => void settings.update({ compactCards: true })}>{`kompakt:${settings.values?.compactCards}`}</button>;
    }
    render_(host, <Settings />);
    fireEvent.click(screen.getByRole('button', { name: 'kompakt:false' }));
    expect(host.settings.update).toHaveBeenCalledWith({ compactCards: true });
  });

  it('useColorScheme liefert das Farbschema', () => {
    function Scheme() {
      const scheme = useColorScheme();
      return <span>{`${scheme.scheme}/${scheme.themeId}`}</span>;
    }
    render_(fakeHost(), <Scheme />);
    expect(screen.getByText('dark/dark')).toBeTruthy();
  });

  it('useNavigate wechselt über den Host die Route', () => {
    const host = fakeHost();
    function Go() {
      const navigate = useNavigate();
      return <button onClick={() => navigate('/p/p1/docs')}>los</button>;
    }
    render_(host, <Go />);
    fireEvent.click(screen.getByRole('button', { name: 'los' }));
    expect(host.navigate).toHaveBeenCalledWith('/p/p1/docs');
  });

  it('useOpenSearch öffnet über den Host die Suche', () => {
    const host = fakeHost();
    function Find() {
      const openSearch = useOpenSearch();
      return <button onClick={openSearch}>suchen</button>;
    }
    render_(host, <Find />);
    fireEvent.click(screen.getByRole('button', { name: 'suchen' }));
    expect(host.openSearch).toHaveBeenCalled();
  });

  it.each([
    ['useNav', useNav],
    ['useBoards', useBoards],
    ['useUsage', useUsage],
    ['useSettings', useSettings],
    ['useColorScheme', useColorScheme],
    ['useNavigate', useNavigate],
    ['useOpenSearch', useOpenSearch],
  ] as const)('%s wirft ohne Host-Provider einen klaren Fehler', (name, hook) => {
    function Probe() {
      hook();
      return null;
    }
    vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() => render(<Probe />)).toThrow(new RegExp(`${name} braucht den ThemeHostProvider des Nexus-Hosts`));
    vi.mocked(console.error).mockRestore();
  });
});

describe('Grenze des SDK', () => {
  it('kein Quelltext unter src importiert aus packages/web', () => {
    const dir = join(import.meta.dirname, '..', 'src');
    for (const file of readdirSync(dir)) {
      const text = readFileSync(join(dir, file), 'utf8');
      expect(text, file).not.toMatch(/from\s+['"][^'"]*(packages\/web|\.\.\/web|@nexus\/web)/);
    }
  });
});
