// @vitest-environment jsdom
// HUD-Block im Stil San Andreas: Faust mit Aufgaben und Fragen, Uhr, Panzerung, Gesundheit, Geld und sechs Sterne
import type { ReactElement } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import { ThemeHostProvider } from '@nexus/theme-sdk';
import { GameContext, type Game } from '../src/data/GameContext.js';
import { SaHud } from '../src/sa/SaHud.js';
import { COUNT_MS } from '../src/shared/useCountUp.js';
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
const filled = () => [...hud().querySelectorAll('.gta-sa-star')].map((star) => star.hasAttribute('data-on'));
const bar = (name: RegExp) => screen.getByRole('progressbar', { name });
const cash = () => hud().querySelector('.gta-sa-cash') as HTMLElement;
const wrap = (game: Game, ui: ReactElement) => (
  <ThemeHostProvider value={fakeHost()}>
    <GameContext.Provider value={game}>{ui}</GameContext.Provider>
  </ThemeHostProvider>
);

describe('SaHud', () => {
  it('Faust mit offenen Aufgaben und Fragen wie die Munitionsanzeige, mit Erklärung', () => {
    renderWithGame(<SaHud />, fakeGame({ tasks: 5, questions: 2 }));
    const weapon = hud().querySelector('.gta-sa-weapon') as HTMLElement;
    expect(weapon.querySelector('svg')?.getAttribute('aria-hidden')).toBe('true');
    expect(weapon.querySelector('.gta-sa-ammo')?.textContent).toBe('5-2');
    expect(weapon.getAttribute('title')).toBe('5 offene Aufgaben, 2 offene Fragen');
  });

  it('Uhr und Geld im Format von San Andreas', () => {
    renderWithGame(<SaHud />, fakeGame({ moneyShown: 184_000 }));
    expect(hud().querySelector('.gta-sa-clock')?.textContent).toBe('09:41');
    expect(cash().textContent).toBe('$00184000');
  });

  it('Panzerung zeigt den Rest des 7-Tage-, Gesundheit den des 5-Stunden-Kontingents', () => {
    renderWithGame(<SaHud />, fakeGame({ quota5h: 0.62, quota7d: 0.81 }));
    expect(bar(/Panzerung/).getAttribute('aria-valuenow')).toBe('81');
    expect(bar(/Panzerung/).style.getPropertyValue('--sa-p')).toBe('81%');
    expect(bar(/Gesundheit/).getAttribute('aria-valuenow')).toBe('62');
    expect(bar(/Gesundheit/).classList.contains('gta-sa-health')).toBe(true);
    expect(bar(/Panzerung/).classList.contains('gta-sa-armor')).toBe(true);
    expect(bar(/Panzerung/).getAttribute('aria-label')).toBe('Panzerung: Rest des 7-Tage-Kontingents');
    expect(bar(/Panzerung/).getAttribute('title')).toBe('Panzerung (7-Tage-Kontingent): 81 % frei');
    expect(bar(/Gesundheit/).getAttribute('title')).toBe('Gesundheit (5-Stunden-Kontingent): 62 % frei');
  });

  it('Balken ohne Wert sind leer, gedimmt und erklären sich im title', () => {
    renderWithGame(<SaHud />, fakeGame({ quota5h: null, quota7d: null }));
    for (const name of [/Panzerung/, /Gesundheit/]) {
      expect(bar(name).hasAttribute('data-empty')).toBe(true);
      expect(bar(name).hasAttribute('aria-valuenow')).toBe(false);
      expect(bar(name).style.getPropertyValue('--sa-p')).toBe('0%');
      expect(bar(name).getAttribute('title')).toBe('Claude-Verbrauch nicht verfügbar');
    }
  });

  it('sechs Sterne, von rechts nach wanted gefüllt', () => {
    renderWithGame(<SaHud />, fakeGame({ wanted: 2 }));
    expect(filled()).toEqual([false, false, false, false, true, true]);
    expect(screen.getByRole('img', { name: 'Fahndungslevel 2 von 6' })).toBeTruthy();
    expect(hud().querySelector('.gta-sa-stars')?.hasAttribute('data-blink')).toBe(false);
  });

  it('bei sechs Sternen blinken sie; mehr als sechs gibt es nicht', () => {
    renderWithGame(<SaHud />, fakeGame({ wanted: 9 }));
    expect(filled()).toEqual([true, true, true, true, true, true]);
    expect(hud().querySelector('.gta-sa-stars')?.hasAttribute('data-blink')).toBe(true);
  });

  it('zählt das Geld bei Änderung hoch und landet genau auf dem neuen Betrag', () => {
    vi.useFakeTimers();
    const game = fakeGame({ moneyShown: 100_000 });
    const view = render(wrap(game, <SaHud />));
    view.rerender(wrap({ ...game, moneyShown: 140_000 }, <SaHud />));
    act(() => vi.advanceTimersByTime(COUNT_MS / 2));
    const middle = Number(cash().textContent!.replace(/\D/g, ''));
    expect(middle).toBeGreaterThan(100_000);
    expect(middle).toBeLessThan(140_000);
    expect(cash().hasAttribute('data-up')).toBe(true);
    act(() => vi.advanceTimersByTime(COUNT_MS));
    expect(cash().textContent).toBe('$00140000');
    expect(cash().hasAttribute('data-up')).toBe(false);
  });

  it('bei reduzierter Bewegung springt das Geld sofort', () => {
    mockMotion(true);
    const game = fakeGame({ moneyShown: 100_000 });
    const view = render(wrap(game, <SaHud />));
    view.rerender(wrap({ ...game, moneyShown: 140_000 }, <SaHud />));
    expect(cash().textContent).toBe('$00140000');
  });
});
