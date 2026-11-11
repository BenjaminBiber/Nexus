// @vitest-environment jsdom
// Minimap unten links: eckige Karte mit den offenen Tickets als Blips, ganz ein Link zum Board, darunter drei Balken
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, screen } from '@testing-library/react';
import { VMinimap } from '../src/v/VMinimap.js';
import { fakeGame, fakeHost, renderWithGame } from './helpers/fakes.js';

afterEach(cleanup);

const radar = () => document.querySelector('[data-gta-part="radar"]') as HTMLElement;
const blips = () => [...radar().querySelectorAll<SVGElement>('[data-blip]')];
const bars = () => screen.getAllByRole('progressbar');

describe('VMinimap', () => {
  it('ist ein Link zum Board, der die Zahl offener Tickets nennt', () => {
    const host = fakeHost();
    renderWithGame(<VMinimap />, fakeGame(), host);
    const link = screen.getByRole('link', { name: 'Minimap: 5 offene Tickets, zum Board' });
    expect(link.getAttribute('href')).toBe('/p/p1/board');
    expect(radar().contains(link)).toBe(true);
    fireEvent.click(link);
    expect(host.navigate).toHaveBeenCalledWith('/p/p1/board');
  });

  it('zeigt je offenem Ticket einen Blip in der Farbe seiner Spalte an seiner Position', () => {
    const game = fakeGame();
    renderWithGame(<VMinimap />, game);
    expect(blips().map((blip) => blip.getAttribute('data-blip'))).toEqual(game.blips.map((b) => b.id));
    for (const blip of game.blips) {
      const el = radar().querySelector(`[data-blip="${blip.id}"]`) as SVGElement;
      expect(el.getAttribute('fill')).toBe(`var(--gta-c-${blip.column})`);
      expect(Number(el.getAttribute('cx'))).toBeCloseTo(blip.x * 188, 1);
      expect(Number(el.getAttribute('cy'))).toBeCloseTo(blip.y * 106, 1);
    }
  });

  it('nur blockierte Blips blinken', () => {
    renderWithGame(<VMinimap />);
    const blinking = blips().filter((blip) => blip.hasAttribute('data-blink')).map((blip) => blip.getAttribute('data-blip'));
    expect(blinking).toEqual(['NOVA-0004']);
  });

  it('nennt die Einzahl richtig und hat ohne Tickets keine Blips', () => {
    renderWithGame(<VMinimap />, fakeGame({ open: 1, blips: fakeGame().blips.slice(0, 1) }));
    expect(screen.getByRole('link', { name: 'Minimap: 1 offenes Ticket, zum Board' })).toBeTruthy();
    cleanup();
    renderWithGame(<VMinimap />, fakeGame({ open: 0, blips: [] }));
    expect(blips()).toHaveLength(0);
  });

  it('drei Balken: Board, 5 Stunden, 7 Tage mit Werten aus dem Spiel', () => {
    renderWithGame(<VMinimap />, fakeGame({ boardProgress: 0.25, quota5h: 0.5, quota7d: 0.8 }));
    expect(bars().map((bar) => bar.getAttribute('aria-label'))).toEqual([
      'Fortschritt des Boards',
      'Claude: Rest des 5-Stunden-Kontingents',
      'Claude: Rest des 7-Tage-Kontingents',
    ]);
    expect(bars().map((bar) => bar.getAttribute('aria-valuenow'))).toEqual(['25', '50', '80']);
    expect(bars().map((bar) => bar.style.getPropertyValue('--v-p'))).toEqual(['25%', '50%', '80%']);
    expect([...radar().querySelectorAll('.gta-v-bar-label')].map((label) => label.textContent)).toEqual(['BOARD', '5 STD', '7 T']);
  });

  it('Balken ohne Wert sind leer, gedimmt und erklären sich im title', () => {
    renderWithGame(<VMinimap />, fakeGame({ boardProgress: null, quota5h: null, quota7d: null }));
    for (const bar of bars()) {
      expect(bar.hasAttribute('data-empty')).toBe(true);
      expect(bar.hasAttribute('aria-valuenow')).toBe(false);
      expect(bar.style.getPropertyValue('--v-p')).toBe('0%');
      expect(bar.getAttribute('title')).toMatch(/nicht verfügbar|noch keine Tickets/);
    }
  });
});
