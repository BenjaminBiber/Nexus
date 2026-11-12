// @vitest-environment jsdom
// Deck-Leiste: VU-Meter für die Claude-Nutzung und Schild „ON AIR“ für die Live-Verbindung
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { ThemeHostProvider, type ThemeHost, type UsageState } from '@nexus/theme-sdk';
import { litSegments, VuMeter } from '../src/VuMeter.js';
import { OnAir } from '../src/OnAir.js';
import { SynthShell } from '../src/SynthShell.js';

afterEach(cleanup);

const OFF: UsageState = { enabled: false, available: false, reason: null, fiveHour: null, sevenDay: null };

function fakeHost(usage: UsageState = OFF): ThemeHost {
  const parts = {
    search: <button type="button">teil-suche</button>,
    boardSwitcher: <span>teil-board</span>,
    usage: <span>teil-verbrauch</span>,
    navItems: {},
    outlet: <span>teil-seite</span>,
    connection: <span>teil-banner</span>,
    status: <span className="live">teil-status</span>,
  };
  return {
    nav: [],
    boards: { current: null, projects: [], boards: [], loading: false, choose: vi.fn() },
    usage,
    settings: { values: null, loading: true, update: vi.fn(async () => {}) },
    themeSettings: { values: {}, loading: false, set: vi.fn(async () => {}) },
    colorScheme: { scheme: 'dark', themeId: 'synthwave', mode: 'synthwave' },
    navigate: vi.fn(),
    openSearch: vi.fn(),
    parts,
  };
}

function usage(percent: number): UsageState {
  return {
    enabled: true,
    available: true,
    reason: null,
    fiveHour: { percent, resetsAt: '2026-11-10T15:30:00' },
    sevenDay: { percent: 20, resetsAt: null },
  };
}

function renderWith(node: React.ReactNode, host = fakeHost()) {
  return render(<ThemeHostProvider value={host}>{node}</ThemeHostProvider>);
}

describe('litSegments', () => {
  it.each([
    [0, 0],
    [1, 1],
    [45, 5],
    [80, 10],
    [100, 12],
    [140, 12],
  ])('%i %% ergibt %i leuchtende Segmente', (percent, lit) => {
    expect(litSegments(percent)).toBe(lit);
  });
});

describe('VU-Meter', () => {
  it.each([
    [0, 0],
    [45, 5],
    [80, 10],
    [100, 12],
  ])('zeigt bei %i %% %i von 12 Segmenten leuchtend', (percent, lit) => {
    renderWith(<VuMeter />, fakeHost(usage(percent)));
    const meter = screen.getByRole('meter', { name: 'Claude-Nutzung 5 Stunden' });
    expect(meter.getAttribute('aria-valuenow')).toBe(String(percent));
    expect(meter.querySelectorAll('i')).toHaveLength(12);
    expect(meter.querySelectorAll('i[data-on]')).toHaveLength(lit);
    expect(meter.querySelectorAll('i[data-zone="warn"]')).toHaveLength(2);
    expect(meter.querySelectorAll('i[data-zone="peak"]')).toHaveLength(2);
  });

  it('nennt den Rücksetzzeitpunkt im title', () => {
    renderWith(<VuMeter />, fakeHost(usage(45)));
    const meter = screen.getByRole('meter', { name: 'Claude-Nutzung 5 Stunden' });
    expect(meter.getAttribute('title')).toContain('15:30');
  });

  it('zeigt das 7-Tage-Fenster als zweite Zeile', () => {
    renderWith(<VuMeter />, fakeHost(usage(45)));
    expect(screen.getByRole('meter', { name: 'Claude-Nutzung 7 Tage' }).getAttribute('aria-valuenow')).toBe('20');
  });

  it('rendert nichts bei abgeschalteter Nutzung', () => {
    const { container } = renderWith(<VuMeter />, fakeHost({ ...usage(45), enabled: false }));
    expect(container.innerHTML).toBe('');
  });

  it('zeigt bei fehlenden Werten kein Meter, nur den Grund', () => {
    renderWith(<VuMeter />, fakeHost({ ...OFF, enabled: true, reason: 'kein Account' }));
    expect(screen.queryByRole('meter')).toBeNull();
    expect(screen.getByTitle('kein Account').textContent).toBe('VU –');
  });
});

describe('Schild „ON AIR“', () => {
  it('umschließt den Verbindungspunkt des Hosts und den Text', () => {
    renderWith(<OnAir />);
    const sign = document.querySelector('.synth-onair') as HTMLElement;
    expect(sign.querySelector('.live')?.textContent).toBe('teil-status');
    expect(sign.querySelector('.synth-onair-text')?.textContent).toBe('ON AIR');
    expect(sign.querySelector('.synth-onair-text')?.getAttribute('aria-hidden')).toBe('true');
  });
});

describe('Deck-Leiste der Shell', () => {
  it('enthält VU-Meter und Schild', () => {
    renderWith(<SynthShell pid="p1" nav={null} content={<main>seite</main>} parts={fakeHost().parts} />, fakeHost(usage(45)));
    const deck = document.querySelector('footer.synth-deck') as HTMLElement;
    expect(deck.querySelector('.synth-vu[role="meter"]')).toBeTruthy();
    expect(deck.querySelector('.synth-onair')).toBeTruthy();
  });
});
