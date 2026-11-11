// @vitest-environment jsdom
// HUD oben rechts im Stil GTA V: Board-Auswahl, Uhr, fünf Sterne und Geld, das bei Änderung hochzählt
import type { ReactElement } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, screen, within } from '@testing-library/react';
import { ThemeHostProvider } from '@nexus/theme-sdk';
import { GameContext, type Game } from '../src/data/GameContext.js';
import { COUNT_MS } from '../src/shared/useCountUp.js';
import { VHud } from '../src/v/VHud.js';
import { fakeGame, fakeHost, renderWithGame } from './helpers/fakes.js';

function mockMotion(reduce: boolean) {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: reduce && query.includes('prefers-reduced-motion'),
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  })) as unknown as typeof window.matchMedia;
}

beforeEach(() => mockMotion(false));

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

const hud = () => document.querySelector('[data-gta-part="hud"]') as HTMLElement;
const filled = () => [...hud().querySelectorAll('.gta-v-star')].map((star) => star.classList.contains('on'));
const wrap = (game: Game, ui: ReactElement) => (
  <ThemeHostProvider value={fakeHost()}>
    <GameContext.Provider value={game}>{ui}</GameContext.Provider>
  </ThemeHostProvider>
);

describe('VHud', () => {
  it('zeigt Board-Auswahl, Uhr im Format von GTA V und das Geld', () => {
    renderWithGame(<VHud />, fakeGame({ moneyShown: 184_000 }));
    expect(within(hud()).getByText('teil-board')).toBeTruthy();
    expect(within(hud()).getByText('Mittwoch 09:41')).toBeTruthy();
    expect(hud().querySelector('.gta-v-cash')?.textContent).toBe('$184.000');
  });

  it('füllt die Sterne von rechts nach wanted und nennt das Fahndungslevel', () => {
    renderWithGame(<VHud />, fakeGame({ wanted: 2 }));
    expect(filled()).toEqual([false, false, false, true, true]);
    expect(screen.getByRole('img', { name: 'Fahndungslevel 2 von 5' })).toBeTruthy();
    expect(hud().querySelector('.gta-v-stars')?.hasAttribute('data-blink')).toBe(false);
  });

  it('zeigt höchstens fünf Sterne; bei fünf und mehr blinken sie', () => {
    renderWithGame(<VHud />, fakeGame({ wanted: 6 }));
    expect(filled()).toEqual([true, true, true, true, true]);
    expect(hud().querySelector('.gta-v-stars')?.hasAttribute('data-blink')).toBe(true);
  });

  it('ohne Fahndung sind alle Sterne leer', () => {
    renderWithGame(<VHud />, fakeGame({ wanted: 0 }));
    expect(filled()).toEqual([false, false, false, false, false]);
  });

  it('zählt das Geld bei Änderung hoch und landet genau auf dem neuen Betrag', () => {
    vi.useFakeTimers();
    const game = fakeGame({ moneyShown: 100_000 });
    const view = render(wrap(game, <VHud />));
    view.rerender(wrap({ ...game, moneyShown: 140_000 }, <VHud />));
    const cash = () => hud().querySelector('.gta-v-cash') as HTMLElement;
    act(() => vi.advanceTimersByTime(COUNT_MS / 2));
    const middle = Number(cash().textContent!.replace(/\D/g, ''));
    expect(middle).toBeGreaterThan(100_000);
    expect(middle).toBeLessThan(140_000);
    expect(cash().hasAttribute('data-up')).toBe(true);
    act(() => vi.advanceTimersByTime(COUNT_MS));
    expect(cash().textContent).toBe('$140.000');
    expect(cash().hasAttribute('data-up')).toBe(false);
  });

  it('ohne Board-Stand ein Platzhalter statt $0; der erste Stand springt, statt hochzuzählen', () => {
    vi.useFakeTimers();
    const game = fakeGame({ loaded: false, moneyShown: 0 });
    const view = render(wrap(game, <VHud />));
    const cash = () => hud().querySelector('.gta-v-cash') as HTMLElement;
    expect(cash().textContent).toBe('$–');
    view.rerender(wrap({ ...game, loaded: true, moneyShown: 140_000 }, <VHud />));
    expect(cash().textContent).toBe('$140.000');
    expect(cash().hasAttribute('data-up')).toBe(false);
  });

  it('bei reduzierter Bewegung springt das Geld sofort', () => {
    mockMotion(true);
    const game = fakeGame({ moneyShown: 100_000 });
    const view = render(wrap(game, <VHud />));
    view.rerender(wrap({ ...game, moneyShown: 140_000 }, <VHud />));
    expect(hud().querySelector('.gta-v-cash')?.textContent).toBe('$140.000');
  });
});
