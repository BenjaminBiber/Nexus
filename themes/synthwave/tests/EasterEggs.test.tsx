// @vitest-environment jsdom
// Versteckte Details des Kassettendecks: Seite A/B, Auto-Reverse, Neonröhre, Sternschnuppe, Overdrive, OFF AIR, STAGE CLEAR
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, renderHook, screen } from '@testing-library/react';
import { ThemeHostProvider, type NavItem, type ThemeHost } from '@nexus/theme-sdk';
import { SIDE_KEY, SIDE_MS, flipSide, resetSide, setSide, useCassetteSide } from '../src/cassette.js';
import { KONAMI, OVERDRIVE_MS, advance, useOverdrive } from '../src/overdrive.js';
import { REVERSE_MS, TapeCounter, sidesPlayed } from '../src/TapeCounter.js';
import { FLICKER_MS, NeonLogo, nextFlickerDelay } from '../src/NeonLogo.js';
import { OnAir } from '../src/OnAir.js';
import { LaserProgress } from '../src/LaserProgress.js';
import { FLIP_MS, TrackList, trackNo } from '../src/TrackList.js';
import { SHOOTING_MS, nextShootingStar } from '../src/Horizon.js';

let reduce = false;

beforeEach(() => {
  reduce = false;
  window.matchMedia = vi.fn(() => ({ matches: reduce })) as unknown as typeof window.matchMedia;
  window.localStorage.clear();
  resetSide();
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
  document.documentElement.removeAttribute('data-synth-overdrive');
});

const NAV: NavItem[] = [
  { id: 'board', label: 'Board', href: '/p/p1/board', active: true },
  { id: 'dashboard', label: 'Dashboard', href: '/p/p1/dashboard', active: false },
];

function host(): ThemeHost {
  return {
    nav: NAV,
    boards: { current: null, projects: [], boards: [], loading: false, choose: vi.fn() },
    usage: { enabled: false, available: false, reason: null, fiveHour: null, sevenDay: null },
    settings: { values: null, loading: true, update: vi.fn(async () => {}) },
    themeSettings: { values: {}, loading: false, set: vi.fn(async () => {}) },
    colorScheme: { scheme: 'dark', themeId: 'synthwave', mode: 'synthwave' },
    navigate: vi.fn(),
    openSearch: vi.fn(),
    parts: {
      search: <span />,
      boardSwitcher: <span />,
      usage: <span />,
      navItems: {},
      outlet: <span />,
      connection: <span className="live off">getrennt</span>,
      status: <span />,
    },
  };
}

describe('Kassette', () => {
  it('liegt ohne gemerkte Seite auf A, umdrehen ergibt B und wird gemerkt', () => {
    const { result } = renderHook(() => useCassetteSide());
    expect(result.current).toBe('A');
    act(() => flipSide());
    expect(result.current).toBe('B');
    expect(window.localStorage.getItem(SIDE_KEY)).toBe('B');
    act(() => flipSide());
    expect(result.current).toBe('A');
  });

  it('liest die gemerkte Seite beim Laden', () => {
    window.localStorage.setItem(SIDE_KEY, 'B');
    const { result } = renderHook(() => useCassetteSide());
    expect(result.current).toBe('B');
  });

  it('Titelnummern folgen der Seite', () => {
    expect(trackNo('A', 0)).toBe('A1');
    expect(trackNo('B', 3)).toBe('B4');
  });

  it('der Knopf „SIDE A“ dreht die Kassette, die Titel heißen dann B1, B2 …', () => {
    const { container } = render(
      <ThemeHostProvider value={host()}>
        <TrackList />
      </ThemeHostProvider>,
    );
    const button = screen.getByRole('button', { name: /Kassette umdrehen, jetzt Seite A/ });
    expect(button.textContent).toBe('SIDE A');
    fireEvent.click(button);
    expect(button.textContent).toBe('SIDE B');
    expect(container.querySelector('.synth-tracks')?.getAttribute('data-side')).toBe('B');
    expect(container.textContent).toContain('B1');
    expect(container.textContent).not.toContain('A1');
  });

  it('dreht sich nur beim Umdrehen, nicht beim Laden mit gemerkter Seite B', () => {
    vi.useFakeTimers();
    window.localStorage.setItem(SIDE_KEY, 'B');
    const { container } = render(
      <ThemeHostProvider value={host()}>
        <TrackList />
      </ThemeHostProvider>,
    );
    const nav = () => container.querySelector('.synth-tracks');
    expect(nav()?.hasAttribute('data-flipping')).toBe(false);
    act(() => flipSide());
    expect(nav()?.hasAttribute('data-flipping')).toBe(true);
    act(() => vi.advanceTimersByTime(FLIP_MS));
    expect(nav()?.hasAttribute('data-flipping')).toBe(false);
  });
});

describe('Auto-Reverse', () => {
  it('zählt volle Seiten einer C90', () => {
    expect(sidesPlayed(0)).toBe(0);
    expect(sidesPlayed(SIDE_MS - 1)).toBe(0);
    expect(sidesPlayed(SIDE_MS)).toBe(1);
    expect(sidesPlayed(-5)).toBe(0);
  });

  it('dreht nach 45 Minuten mit VHS die Kassette und zeigt kurz AUTO REVERSE, danach ◀ PLAY', () => {
    vi.useFakeTimers();
    const { container } = render(<TapeCounter vhs />);
    expect(container.textContent).toContain('PLAY ▶');
    act(() => vi.advanceTimersByTime(SIDE_MS));
    const osd = container.querySelector('.synth-osd');
    expect(osd?.textContent).toContain('AUTO REVERSE');
    expect(osd?.hasAttribute('data-reversing')).toBe(true);
    expect(window.localStorage.getItem(SIDE_KEY)).toBe('B');
    act(() => vi.advanceTimersByTime(REVERSE_MS));
    expect(container.querySelector('.synth-osd')?.textContent).toContain('◀ PLAY');
    expect(container.querySelector('.synth-osd')?.hasAttribute('data-reversing')).toBe(false);
  });

  it('ohne VHS dreht sich nichts', () => {
    vi.useFakeTimers();
    render(<TapeCounter vhs={false} />);
    act(() => vi.advanceTimersByTime(SIDE_MS + 30_000));
    expect(window.localStorage.getItem(SIDE_KEY)).toBeNull();
  });

  it('Seite B zeigt ◀ PLAY', () => {
    act(() => setSide('B'));
    const { container } = render(<TapeCounter vhs />);
    expect(container.textContent).toContain('◀ PLAY');
  });
});

describe('Neonröhre', () => {
  it('Schriftzug besteht aus einzelnen Buchstaben, keiner setzt am Anfang aus', () => {
    const { container } = render(<NeonLogo />);
    const letters = container.querySelectorAll('.synth-logo-letter');
    expect([...letters].map((letter) => letter.textContent).join('')).toBe('Nexus');
    expect(container.querySelector('[data-flicker]')).toBeNull();
  });

  it('nach der Pause setzt ein Buchstabe kurz aus und brennt dann wieder', () => {
    vi.useFakeTimers();
    vi.spyOn(Math, 'random').mockReturnValue(0.5);
    const { container } = render(<NeonLogo />);
    act(() => vi.advanceTimersByTime(nextFlickerDelay(0.5)));
    expect(container.querySelectorAll('[data-flicker]')).toHaveLength(1);
    act(() => vi.advanceTimersByTime(FLICKER_MS));
    expect(container.querySelector('[data-flicker]')).toBeNull();
  });

  it('bei reduzierter Bewegung flackert nichts', () => {
    reduce = true;
    vi.useFakeTimers();
    const { container } = render(<NeonLogo />);
    act(() => vi.advanceTimersByTime(nextFlickerDelay(1) + FLICKER_MS));
    expect(container.querySelector('[data-flicker]')).toBeNull();
  });

  it('Pausen liegen zwischen 25 und 60 s', () => {
    expect(nextFlickerDelay(0)).toBe(25_000);
    expect(nextFlickerDelay(1)).toBe(60_000);
  });
});

describe('Sternschnuppe', () => {
  it('die Pause bis zur nächsten liegt zwischen 20 und 50 s, sie selbst ist kurz', () => {
    expect(nextShootingStar(0)).toBe(20_000);
    expect(nextShootingStar(1)).toBe(50_000);
    expect(SHOOTING_MS).toBeLessThan(2000);
  });
});

describe('Overdrive (Konami-Code)', () => {
  const type = (keys: string[]) => keys.reduce(advance, 0);

  it('erkennt die ganze Folge, auch mit großem B und A', () => {
    expect(type(KONAMI)).toBe(KONAMI.length);
    expect(type([...KONAMI.slice(0, 8), 'B', 'A'])).toBe(KONAMI.length);
  });

  it('ein ↑ zu viel am Anfang schadet nicht, ein falscher Schritt beginnt von vorn', () => {
    expect(type(['ArrowUp', ...KONAMI])).toBe(KONAMI.length);
    expect(type(['ArrowUp', 'ArrowUp', 'ArrowLeft'])).toBe(0);
    expect(type(['ArrowUp', 'ArrowDown', 'ArrowUp'])).toBe(1);
  });

  it('setzt nach dem Code data-synth-overdrive auf <html> und nimmt es nach dem Turbo wieder weg', () => {
    vi.useFakeTimers();
    const { result } = renderHook(() => useOverdrive());
    act(() => {
      for (const key of KONAMI) window.dispatchEvent(new KeyboardEvent('keydown', { key }));
    });
    expect(result.current).toBe(true);
    expect(document.documentElement.hasAttribute('data-synth-overdrive')).toBe(true);
    act(() => vi.advanceTimersByTime(OVERDRIVE_MS));
    expect(result.current).toBe(false);
    expect(document.documentElement.hasAttribute('data-synth-overdrive')).toBe(false);
  });

  it('Tippen in einem Feld zählt nicht', () => {
    const { result } = renderHook(() => useOverdrive());
    const input = document.createElement('input');
    document.body.append(input);
    act(() => {
      for (const key of KONAMI) input.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
    });
    expect(result.current).toBe(false);
    input.remove();
  });

  it('bei reduzierter Bewegung bleibt alles ruhig', () => {
    reduce = true;
    const { result } = renderHook(() => useOverdrive());
    act(() => {
      for (const key of KONAMI) window.dispatchEvent(new KeyboardEvent('keydown', { key }));
    });
    expect(result.current).toBe(false);
  });
});

describe('Schilder', () => {
  it('ON AIR trägt auch die Aufschrift OFF AIR für die getrennte Verbindung', () => {
    const { container } = render(
      <ThemeHostProvider value={host()}>
        <OnAir />
      </ThemeHostProvider>,
    );
    expect(container.querySelector('.synth-onair-text')?.textContent).toBe('ON AIR');
    expect(container.querySelector('.synth-offair-text')?.textContent).toBe('OFF AIR');
    expect(container.querySelector('.synth-offair-text')?.getAttribute('aria-hidden')).toBe('true');
  });

  it('STAGE CLEAR erscheint nur, wenn alle Schritte erledigt sind', () => {
    const { container, rerender } = render(<LaserProgress done={4} total={5} label="Schritte" caption="Schritte" />);
    expect(container.querySelector('.synth-clear')).toBeNull();
    rerender(<LaserProgress done={5} total={5} label="Schritte" caption="Schritte" />);
    expect(container.querySelector('.synth-clear')?.textContent).toBe('STAGE CLEAR');
    rerender(<LaserProgress done={0} total={0} label="Schritte" caption="Schritte" />);
    expect(container.querySelector('.synth-clear')).toBeNull();
  });
});
