// @vitest-environment jsdom
// Platzhalter-Rahmen „GTA“ mit einem Host aus erkennbaren Bausteinen: Attribute auf <html>, gemerkter Stil beim Laden
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, renderHook, within } from '@testing-library/react';
import { ThemeHostProvider, type ThemeHost } from '@nexus/theme-sdk';
import { GtaShell } from '../src/GtaShell.js';
import { BOOT_KEY, STYLE_KEY, useActiveStyle } from '../src/settings.js';
import theme from '../src/main.js';

afterEach(() => {
  cleanup();
  window.localStorage.clear();
});

// Host mit Bausteinen und gespeicherten Einstellungen des Themes; loading = Einstellungen laden noch
function fakeHost(values: Record<string, boolean | string> = {}, loading = false): ThemeHost {
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
    nav: [],
    boards: { current: null, projects: [], boards: [], loading: false, choose: vi.fn() },
    usage: { enabled: false, available: false, reason: null, fiveHour: null, sevenDay: null },
    settings: { values: null, loading: true, update: vi.fn(async () => {}) },
    themeSettings: { values, loading, set: vi.fn(async () => {}) },
    colorScheme: { scheme: 'dark', themeId: 'gta', mode: 'gta' },
    navigate: vi.fn(),
    openSearch: vi.fn(),
    parts,
  };
}

function renderShell(values: Record<string, boolean | string> = {}, loading = false) {
  const host = fakeHost(values, loading);
  return render(
    <ThemeHostProvider value={host}>
      <GtaShell pid="p1" nav={<a href="/p/p1/board">nav-board</a>} content={<main>seiten-inhalt</main>} parts={host.parts} />
    </ThemeHostProvider>,
  );
}

const attr = (name: string) => document.documentElement.getAttribute(name);

describe('Rahmen „GTA“ (Platzhalter)', () => {
  it('ist als Slot Shell angemeldet', () => {
    expect(theme.id).toBe('gta');
    expect(theme.slots.Shell).toBe(GtaShell);
  });

  it('zeigt oben Wortmarke, Board-Auswahl und Suche, darunter Navigation und Inhalt', () => {
    renderShell();
    const shell = document.querySelector('[data-gta-part="shell"]') as HTMLElement;
    expect(shell.className).toBe('gta-screen');
    const header = shell.querySelector('header') as HTMLElement;
    expect(within(header).getByRole('img', { name: 'Nexus' }).getAttribute('data-wordmark')).toBe('nexus');
    expect(within(header).getByText('teil-board')).toBeTruthy();
    expect(within(header).getByRole('button', { name: 'teil-suche' })).toBeTruthy();
    const nav = shell.querySelector('nav.gta-nav') as HTMLElement;
    expect(within(nav).getByRole('link', { name: 'nav-board' }).getAttribute('href')).toBe('/p/p1/board');
    expect(within(shell).getByRole('main').textContent).toBe('seiten-inhalt');
  });

  it('legt Stil, Akzent und Radar als data-gta-* auf <html> und räumt sie beim Abbau weg', () => {
    const { unmount } = renderShell({ style: 'sa', accent: 'orange', radar: false });
    expect(attr('data-gta-style')).toBe('sa');
    expect(attr('data-gta-accent')).toBe('orange');
    expect(attr('data-gta-radar')).toBe('off');
    unmount();
    for (const name of ['style', 'accent', 'radar']) expect(attr(`data-gta-${name}`), name).toBeNull();
  });

  it('während die Einstellungen laden, gilt der gemerkte Stil und nichts wird überschrieben', () => {
    window.localStorage.setItem(STYLE_KEY, 'sa');
    window.localStorage.setItem(BOOT_KEY, 'off');
    renderShell({}, true);
    expect(attr('data-gta-style')).toBe('sa');
    expect(window.localStorage.getItem(STYLE_KEY)).toBe('sa');
    expect(window.localStorage.getItem(BOOT_KEY)).toBe('off');
  });

  it('merkt Stil und Ladebildschirm-Schalter nach dem Laden', () => {
    renderShell({ style: 'sa', boot: false });
    expect(window.localStorage.getItem(STYLE_KEY)).toBe('sa');
    expect(window.localStorage.getItem(BOOT_KEY)).toBe('off');
    cleanup();
    renderShell();
    expect(attr('data-gta-style')).toBe('v');
    expect(window.localStorage.getItem(STYLE_KEY)).toBe('v');
    expect(window.localStorage.getItem(BOOT_KEY)).toBe('on');
  });
});

describe('useActiveStyle', () => {
  const wrapper = (host: ThemeHost) => ({ children }: { children: ReactNode }) =>
    createElement(ThemeHostProvider, { value: host }, children);

  it('liefert beim Laden den gemerkten Stil, danach die Einstellung', () => {
    window.localStorage.setItem(STYLE_KEY, 'sa');
    expect(renderHook(() => useActiveStyle(), { wrapper: wrapper(fakeHost({}, true)) }).result.current).toBe('sa');
    expect(renderHook(() => useActiveStyle(), { wrapper: wrapper(fakeHost({ style: 'v' })) }).result.current).toBe('v');
  });
});
