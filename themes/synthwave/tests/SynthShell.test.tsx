// @vitest-environment jsdom
// Rahmen „Kassettendeck“ mit einem Host aus erkennbaren Bausteinen
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import { ThemeHostProvider, type NavItem, type ThemeHost } from '@nexus/theme-sdk';
import { SynthShell } from '../src/SynthShell.js';
import theme from '../src/main.js';

afterEach(cleanup);

const NAV: NavItem[] = [
  { id: 'board', label: 'Board', href: '/p/p1/board', active: false },
  { id: 'inbox', label: 'Meine Aufgaben', href: '/p/p1/inbox', active: false, badge: 3 },
  { id: 'dashboard', label: 'Dashboard', href: '/p/p1/dashboard', active: false },
  { id: 'sessions', label: 'Sessions', href: '/p/p1/sessions', active: true },
  { id: 'settings', label: 'Einstellungen', href: '/p/p1/settings', active: false },
];

// Host mit Navigation, Bausteinen und gespeicherten Einstellungen des Themes
function fakeHost(values: Record<string, boolean | string> = {}): ThemeHost {
  const parts = {
    search: <button type="button">teil-suche</button>,
    boardSwitcher: <span>teil-board</span>,
    usage: <span>teil-verbrauch</span>,
    navItems: {},
    outlet: <span>teil-seite</span>,
    connection: <span>teil-banner</span>,
    status: <span>teil-status</span>,
  };
  return {
    nav: NAV,
    boards: { current: null, projects: [], boards: [], loading: false, choose: vi.fn() },
    usage: { enabled: false, available: false, reason: null, fiveHour: null, sevenDay: null },
    settings: { values: null, loading: true, update: vi.fn(async () => {}) },
    themeSettings: { values, loading: false, set: vi.fn(async () => {}) },
    colorScheme: { scheme: 'dark', themeId: 'synthwave', mode: 'synthwave' },
    navigate: vi.fn(),
    openSearch: vi.fn(),
    parts,
  };
}

function renderShell(values: Record<string, boolean | string> = {}) {
  const host = fakeHost(values);
  const result = render(
    <ThemeHostProvider value={host}>
      <SynthShell pid="p1" nav={null} content={<main>seiten-inhalt</main>} parts={host.parts} />
    </ThemeHostProvider>,
  );
  return { host, ...result };
}

describe('Rahmen „Kassettendeck“', () => {
  it('ist als Slot Shell angemeldet', () => {
    expect(theme.slots.Shell).toBe(SynthShell);
  });

  it('zeigt oben das Display mit Schriftzug, Board-Auswahl und Suche', () => {
    renderShell();
    const display = document.querySelector('header.synth-display') as HTMLElement;
    expect(display).toBeTruthy();
    expect(display.querySelector('.synth-logo')?.textContent).toBe('Nexus');
    expect(display.querySelector('.synth-logo')?.getAttribute('aria-hidden')).toBe('true');
    expect(within(display).getByText('teil-board')).toBeTruthy();
    expect(within(display).getByRole('button', { name: 'teil-suche' })).toBeTruthy();
  });

  it('listet die Navigation als nummerierte Titel in der Reihenfolge des Hosts', () => {
    renderShell();
    const nav = screen.getByRole('navigation', { name: 'Hauptnavigation' });
    expect(nav.querySelector('.synth-side')?.textContent).toBe('SIDE A');
    const links = within(nav).getAllByRole('link');
    expect(links.map((a) => a.getAttribute('href'))).toEqual(NAV.map((item) => item.href));
    expect(links.map((a) => a.querySelector('.synth-track-no')?.textContent)).toEqual(['01', '02', '03', '04', '05']);
    expect(within(nav).getByRole('link', { name: /Board/ })).toBeTruthy();
    expect(within(links[1]!).getByText('3')).toBeTruthy();
  });

  it('markiert den aktiven Titel mit aria-current und Equalizer, die anderen nicht', () => {
    renderShell();
    const links = within(screen.getByRole('navigation', { name: 'Hauptnavigation' })).getAllByRole('link');
    const active = links.filter((a) => a.getAttribute('aria-current') === 'page');
    expect(active.map((a) => a.getAttribute('href'))).toEqual(['/p/p1/sessions']);
    expect(active[0]!.className).toContain('active');
    expect(active[0]!.querySelector('.synth-eq')?.getAttribute('aria-hidden')).toBe('true');
    expect(active[0]!.querySelectorAll('.synth-eq i')).toHaveLength(3);
    expect(links.filter((a) => a.querySelector('.synth-eq'))).toHaveLength(1);
  });

  it('rendert den Inhalt im Hauptbereich und die Deck-Leiste mit dem aktiven Bereich', () => {
    renderShell();
    expect(document.querySelector('.synth-body')?.textContent).toContain('seiten-inhalt');
    const deck = document.querySelector('footer.synth-deck') as HTMLElement;
    expect(deck.getAttribute('data-section')).toBe('sessions');
    expect(deck.querySelector('.synth-tape-label')?.textContent).toBe('Sessions');
  });

  it('legt die Einstellungen als data-synth-* auf <html> und räumt sie beim Abbau weg', () => {
    const { unmount } = renderShell({ palette: 'miami', glow: 'high', horizon: 'still', vhs: true });
    const root = document.documentElement;
    expect(root.getAttribute('data-synth-palette')).toBe('miami');
    expect(root.getAttribute('data-synth-glow')).toBe('high');
    expect(root.getAttribute('data-synth-horizon')).toBe('still');
    expect(root.getAttribute('data-synth-vhs')).toBe('on');
    unmount();
    for (const name of ['palette', 'glow', 'horizon', 'vhs']) expect(root.getAttribute(`data-synth-${name}`), name).toBeNull();
  });
});
