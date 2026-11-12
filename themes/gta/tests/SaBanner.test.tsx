// @vitest-environment jsdom
// Banner im Stil San Andreas: Mission Passed mit Betrag und Respect, Busted, Wasted; Klick oder Esc schließt
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, screen } from '@testing-library/react';
import type { GameEvent } from '../src/data/events.js';
import { SaBanner } from '../src/sa/SaBanner.js';
import { fakeGame, renderWithGame } from './helpers/fakes.js';

afterEach(cleanup);

const banner = () => document.querySelector('[data-gta-part="banner"]') as HTMLElement | null;
const wordmarks = () => [...banner()!.querySelectorAll('[data-wordmark]')].map((wm) => wm.getAttribute('data-wordmark'));

const PASSED: GameEvent = { kind: 'passed', ticketId: 'NOVA-0003', title: 'Kasse', reward: 40_000 };
const BUSTED: GameEvent = { kind: 'busted', ticketId: 'NOVA-0004', title: 'Zahlungsanbieter', reason: 'Zugang zur Sandbox fehlt' };

describe('SaBanner', () => {
  it('ohne Banner steht nur die leere Statushülle', () => {
    renderWithGame(<SaBanner />);
    expect(banner()).toBeNull();
    expect(screen.getByRole('status').textContent).toBe('');
  });

  it('Mission Passed: goldene Wortmarke, Betrag ohne führende Nullen, Respect und darunter Ticket und Titel', () => {
    renderWithGame(<SaBanner />, fakeGame({ banner: PASSED }));
    expect(screen.getByRole('status').contains(banner())).toBe(true);
    expect(banner()!.getAttribute('data-kind')).toBe('passed');
    expect(wordmarks()).toEqual(['missionPassedSa', 'respect']);
    expect(screen.getByRole('img', { name: 'Mission passed!' })).toBeTruthy();
    expect(banner()!.querySelector('.gta-sa-amount')?.textContent).toBe('$40000');
    expect(banner()!.querySelector('.gta-sa-banner-note')?.textContent).toBe('NOVA-0003 · Kasse');
  });

  it('Busted: Wortmarke, Titel und Blockiert-Grund', () => {
    renderWithGame(<SaBanner />, fakeGame({ banner: BUSTED }));
    expect(wordmarks()).toEqual(['busted']);
    expect(screen.getByRole('img', { name: 'Busted' })).toBeTruthy();
    expect(banner()!.querySelector('.gta-sa-banner-text')?.textContent).toBe('NOVA-0004 · Zahlungsanbieter: Zugang zur Sandbox fehlt');
  });

  it('Busted ohne Grund nennt nur Ticket und Titel', () => {
    renderWithGame(<SaBanner />, fakeGame({ banner: { ...BUSTED, reason: null } }));
    expect(banner()!.querySelector('.gta-sa-banner-text')?.textContent).toBe('NOVA-0004 · Zahlungsanbieter');
  });

  it('Wasted: Wortmarke und Hinweis auf das Kontingent', () => {
    renderWithGame(<SaBanner />, fakeGame({ banner: { kind: 'wasted' } }));
    expect(wordmarks()).toEqual(['wasted']);
    expect(banner()!.querySelector('.gta-sa-banner-text')?.textContent).toBe('5-Stunden-Kontingent aufgebraucht');
  });

  it('Klick auf das Banner schließt es', () => {
    const game = fakeGame({ banner: PASSED });
    renderWithGame(<SaBanner />, game);
    fireEvent.click(banner()!.querySelector('.gta-sa-banner-in')!);
    expect(game.dismissBanner).toHaveBeenCalledTimes(1);
  });

  it('Esc schließt das Banner, ohne die Taste abzufangen; ein schon verarbeitetes Esc nicht', () => {
    const game = fakeGame({ banner: BUSTED });
    renderWithGame(<SaBanner />, game);
    const handled = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true });
    handled.preventDefault();
    window.dispatchEvent(handled);
    expect(game.dismissBanner).not.toHaveBeenCalled();
    const esc = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true });
    window.dispatchEvent(esc);
    expect(game.dismissBanner).toHaveBeenCalledTimes(1);
    expect(esc.defaultPrevented).toBe(false);
  });
});
