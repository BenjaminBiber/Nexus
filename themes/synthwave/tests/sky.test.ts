// @vitest-environment jsdom
// Himmel nach Tageszeit: Phasengrenzen, feste Einstellung, Wechsel bei laufender Uhr und die Horizont-Ebene im Rahmen
import { createElement } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, renderHook } from '@testing-library/react';
import { ThemeHostProvider, type ThemeHost } from '@nexus/theme-sdk';
import { skyPhase, useSkyPhase } from '../src/sky.js';
import { SynthShell } from '../src/SynthShell.js';

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

const at = (hours: number, minutes: number) => new Date(2026, 10, 10, hours, minutes);

function fakeHost(values: Record<string, boolean | string> = {}): ThemeHost {
  const parts = {
    search: createElement('button', { type: 'button' }, 'teil-suche'),
    boardSwitcher: createElement('span', null, 'teil-board'),
    usage: createElement('span', null, 'teil-verbrauch'),
    navItems: {},
    outlet: createElement('span', null, 'teil-seite'),
    connection: createElement('span', null, 'teil-banner'),
    status: createElement('span', null, 'teil-status'),
  };
  return {
    nav: [],
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
  return render(
    createElement(
      ThemeHostProvider,
      { value: host },
      createElement(SynthShell, { pid: 'p1', nav: null, content: createElement('main', null, 'seite'), parts: host.parts }),
    ),
  );
}

describe('skyPhase', () => {
  it.each([
    [4, 59, 'night'],
    [5, 0, 'dawn'],
    [8, 59, 'dawn'],
    [9, 0, 'day'],
    [16, 59, 'day'],
    [17, 0, 'dusk'],
    [20, 59, 'dusk'],
    [21, 0, 'night'],
  ] as const)('%i:%i Uhr ist %s', (hours, minutes, phase) => {
    expect(skyPhase(at(hours, minutes))).toBe(phase);
  });
});

describe('useSkyPhase', () => {
  it('folgt bei „auto“ der laufenden Uhr', () => {
    vi.useFakeTimers();
    vi.setSystemTime(at(20, 59));
    const { result } = renderHook(() => useSkyPhase('auto'));
    expect(result.current).toBe('dusk');
    act(() => {
      vi.advanceTimersByTime(2 * 60_000);
    });
    expect(result.current).toBe('night');
  });

  it('eine feste Einstellung gewinnt gegen die Uhrzeit', () => {
    vi.useFakeTimers();
    vi.setSystemTime(at(23, 0));
    const { result } = renderHook(() => useSkyPhase('day'));
    expect(result.current).toBe('day');
    act(() => {
      vi.advanceTimersByTime(2 * 60_000);
    });
    expect(result.current).toBe('day');
  });
});

describe('Horizont im Rahmen', () => {
  it('rendert die Ebene hinter dem Inhalt und setzt data-synth-sky', () => {
    const { unmount } = renderShell({ sky: 'night' });
    const horizon = document.querySelector('.synth-body > .synth-horizon') as HTMLElement;
    expect(horizon).toBeTruthy();
    expect(horizon.getAttribute('aria-hidden')).toBe('true');
    expect(horizon.parentElement?.firstElementChild).toBe(horizon);
    for (const part of ['synth-stars', 'synth-sun', 'synth-grid']) expect(horizon.querySelector(`.${part}`), part).toBeTruthy();
    expect(document.documentElement.getAttribute('data-synth-sky')).toBe('night');
    unmount();
    expect(document.documentElement.getAttribute('data-synth-sky')).toBeNull();
  });

  it('lässt die Ebene bei Horizont „Aus“ weg', () => {
    renderShell({ horizon: 'off' });
    expect(document.querySelector('.synth-horizon')).toBeNull();
  });
});
