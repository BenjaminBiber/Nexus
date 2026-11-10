// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { BoardSwitcher, ConnectionStatus, NavLink, Outlet, SearchButton, ThemeHostProvider, UsageMeter, useShellParts } from '@nexus/theme-sdk';
import type { ThemeHost } from '@nexus/theme-sdk';

afterEach(cleanup);

// Host mit erkennbaren Bausteinen und beobachtbarem Routenwechsel
function fakeHost(): ThemeHost {
  return {
    nav: [
      { id: 'board', label: 'Board', href: '/p/p1/board', active: true },
      { id: 'inbox', label: 'Meine Aufgaben', href: '/p/p1/inbox', active: false, badge: 3 },
    ],
    boards: { current: null, projects: [], boards: [], loading: false, choose: vi.fn() },
    usage: { enabled: false, available: false, reason: null, fiveHour: null, sevenDay: null, accounts: [], defaultLimit: 90 },
    settings: { values: null, loading: true, update: vi.fn(async () => {}) },
    themeSettings: { values: {}, loading: false, set: vi.fn(async () => {}) },
    colorScheme: { scheme: 'dark', themeId: 'dark', mode: 'dark' },
    connection: { connected: true },
    navigate: vi.fn(),
    openSearch: vi.fn(),
    parts: {
      search: <span>teil-suche</span>,
      boardSwitcher: <span>teil-board</span>,
      usage: <span>teil-verbrauch</span>,
      navItems: { board: <span>teil-nav-board</span> },
      outlet: <span>teil-seite</span>,
      connection: <span>teil-banner</span>,
      status: <span>teil-status</span>,
    },
  };
}

function render_(host: ThemeHost, ui: React.ReactNode) {
  return render(<ThemeHostProvider value={host}>{ui}</ThemeHostProvider>);
}

describe('NavLink', () => {
  it('zeigt Label, Ziel und aria-current des aktiven Eintrags', () => {
    render_(fakeHost(), <NavLink id="board" />);
    const link = screen.getByRole('link', { name: 'Board' });
    expect(link.getAttribute('href')).toBe('/p/p1/board');
    expect(link.getAttribute('aria-current')).toBe('page');
  });

  it('zeigt den Zähler und kein aria-current bei inaktivem Eintrag', () => {
    render_(fakeHost(), <NavLink id="inbox" />);
    const link = screen.getByRole('link', { name: /Meine Aufgaben/ });
    expect(link.getAttribute('aria-current')).toBeNull();
    expect(link.textContent).toContain('3');
  });

  it('einfacher Klick wechselt über den Host die Route, Strg-Klick nicht', () => {
    const host = fakeHost();
    render_(host, <NavLink id="inbox" />);
    const link = screen.getByRole('link', { name: /Meine Aufgaben/ });
    fireEvent.click(link, { ctrlKey: true });
    expect(host.navigate).not.toHaveBeenCalled();
    fireEvent.click(link);
    expect(host.navigate).toHaveBeenCalledWith('/p/p1/inbox');
  });

  it('rendert mit children als Funktion ein eigenes Aussehen', () => {
    render_(fakeHost(), <NavLink id="board">{(item) => <b>{`eigen:${item.label}`}</b>}</NavLink>);
    expect(screen.getByRole('link', { name: 'eigen:Board' })).toBeTruthy();
  });

  it('reicht role durch und ruft onClick vor dem Routenwechsel', () => {
    const host = fakeHost();
    const onClick = vi.fn();
    render_(host, <NavLink id="inbox" role="menuitem" onClick={onClick} />);
    fireEvent.click(screen.getByRole('menuitem', { name: /Meine Aufgaben/ }));
    expect(onClick).toHaveBeenCalled();
    expect(host.navigate).toHaveBeenCalledWith('/p/p1/inbox');
  });

  it('rendert bei unbekannter ID nichts', () => {
    const { container } = render_(fakeHost(), <NavLink id="docs" />);
    expect(container.innerHTML).toBe('');
  });
});

describe('Bausteine des Hosts', () => {
  it('rendern das jeweilige Element des Hosts', () => {
    function Parts() {
      return <>{useShellParts().navItems.board}</>;
    }
    render_(fakeHost(), <><BoardSwitcher /><SearchButton /><UsageMeter /><Outlet /><ConnectionStatus /><Parts /></>);
    for (const text of ['teil-board', 'teil-suche', 'teil-verbrauch', 'teil-seite', 'teil-status', 'teil-nav-board']) {
      expect(screen.getByText(text)).toBeTruthy();
    }
  });

  it.each([
    ['NavLink', () => <NavLink id="board" />],
    ['BoardSwitcher', () => <BoardSwitcher />],
    ['SearchButton', () => <SearchButton />],
    ['UsageMeter', () => <UsageMeter />],
    ['Outlet', () => <Outlet />],
    ['ConnectionStatus', () => <ConnectionStatus />],
  ] as const)('%s wirft ohne Host-Provider einen klaren Fehler', (_name, ui) => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() => render(ui())).toThrow(/braucht den ThemeHostProvider des Nexus-Hosts/);
    spy.mockRestore();
  });
});
