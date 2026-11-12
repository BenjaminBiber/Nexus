// @vitest-environment jsdom
// Banner über dem Inhalt: Mission Passed, Busted, Wasted; Klick oder Esc schließt
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, screen } from '@testing-library/react';
import type { GameEvent } from '../src/data/events.js';
import { VBanner } from '../src/v/VBanner.js';
import { fakeGame, renderWithGame } from './helpers/fakes.js';

afterEach(cleanup);

const banner = () => document.querySelector('[data-gta-part="banner"]') as HTMLElement | null;
const wordmarks = () => [...banner()!.querySelectorAll('[data-wordmark]')].map((wm) => wm.getAttribute('data-wordmark'));

const PASSED: GameEvent = { kind: 'passed', ticketId: 'NOVA-0003', title: 'Kasse', reward: 40_000 };
const BUSTED: GameEvent = { kind: 'busted', ticketId: 'NOVA-0004', title: 'Zahlungsanbieter', reason: 'Zugang zur Sandbox fehlt' };

describe('VBanner', () => {
  it('ohne Banner steht nur die leere Statushülle', () => {
    renderWithGame(<VBanner />);
    expect(banner()).toBeNull();
    expect(screen.getByRole('status').textContent).toBe('');
  });

  it('Mission Passed: Wortmarke, Titel in Schreibschrift, Auftrag und Belohnung mit Haken, Medaille und Respect', () => {
    renderWithGame(<VBanner />, fakeGame({ banner: PASSED }));
    expect(screen.getByRole('status').contains(banner())).toBe(true);
    expect(banner()!.getAttribute('data-kind')).toBe('passed');
    expect(wordmarks()).toEqual(['missionPassed', 'respect']);
    expect(screen.getByRole('img', { name: 'Mission Passed' })).toBeTruthy();
    expect(banner()!.querySelector('.gta-v-banner-title')?.textContent).toBe('Kasse');
    const rows = [...banner()!.querySelectorAll('.gta-v-result')].map((row) => [
      row.querySelector('span')?.textContent,
      row.querySelector('b')?.textContent,
      row.querySelector('.gta-v-check') !== null,
    ]);
    expect(rows).toEqual([
      ['Auftrag', 'NOVA-0003', true],
      ['Belohnung', '$40.000', true],
    ]);
    expect(banner()!.querySelector('.gta-v-medal')).not.toBeNull();
  });

  it('Busted: Wortmarke, Titel und Blockiert-Grund', () => {
    renderWithGame(<VBanner />, fakeGame({ banner: BUSTED }));
    expect(wordmarks()).toEqual(['busted']);
    expect(screen.getByRole('img', { name: 'Busted' })).toBeTruthy();
    expect(banner()!.querySelector('.gta-v-banner-title')?.textContent).toBe('Zahlungsanbieter');
    expect(banner()!.textContent).toContain('Zugang zur Sandbox fehlt');
  });

  it('Busted ohne Grund lässt die Zeile weg', () => {
    renderWithGame(<VBanner />, fakeGame({ banner: { ...BUSTED, reason: null } }));
    expect(banner()!.querySelector('.gta-v-banner-text')).toBeNull();
  });

  it('Wasted: Wortmarke und Hinweis auf das Kontingent', () => {
    renderWithGame(<VBanner />, fakeGame({ banner: { kind: 'wasted' } }));
    expect(wordmarks()).toEqual(['wasted']);
    expect(banner()!.textContent).toContain('5-Stunden-Kontingent aufgebraucht');
  });

  it('Klick auf das Band schließt das Banner', () => {
    const game = fakeGame({ banner: PASSED });
    renderWithGame(<VBanner />, game);
    fireEvent.click(banner()!.querySelector('.gta-v-band')!);
    expect(game.dismissBanner).toHaveBeenCalledTimes(1);
  });

  it('Esc schließt das Banner, ohne die Taste abzufangen; andere Tasten nicht', () => {
    const game = fakeGame({ banner: BUSTED });
    renderWithGame(<VBanner />, game);
    const tab = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true });
    window.dispatchEvent(tab);
    expect(game.dismissBanner).not.toHaveBeenCalled();
    expect(tab.defaultPrevented).toBe(false);
    const esc = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true });
    window.dispatchEvent(esc);
    expect(game.dismissBanner).toHaveBeenCalledTimes(1);
    expect(esc.defaultPrevented).toBe(false);
  });

  it('ein Esc, das schon ein Dialog verarbeitet hat, lässt das Banner stehen', () => {
    const game = fakeGame({ banner: BUSTED });
    renderWithGame(<VBanner />, game);
    const esc = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true });
    esc.preventDefault();
    window.dispatchEvent(esc);
    expect(game.dismissBanner).not.toHaveBeenCalled();
  });
});
