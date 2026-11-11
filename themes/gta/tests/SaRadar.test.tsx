// @vitest-environment jsdom
// Rundes Radar im Stil San Andreas: Ring, Nordmarke, Straßen, je offenem Ticket ein Blip, als Ganzes ein Link zum Board
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, screen } from '@testing-library/react';
import { RADAR_INNER, SaRadar } from '../src/sa/SaRadar.js';
import { fakeGame, fakeHost, renderWithGame } from './helpers/fakes.js';

afterEach(cleanup);

const radar = () => document.querySelector('[data-gta-part="radar"]') as HTMLElement;
const blips = () => [...radar().querySelectorAll<SVGElement>('[data-blip]')];

describe('SaRadar', () => {
  it('ist ein Link zum Board, der die Zahl offener Tickets nennt', () => {
    const host = fakeHost();
    renderWithGame(<SaRadar />, fakeGame(), host);
    const link = screen.getByRole('link', { name: 'Radar: 5 offene Tickets, zum Board' });
    expect(link.getAttribute('href')).toBe('/p/p1/board');
    expect(radar().contains(link)).toBe(true);
    fireEvent.click(link);
    expect(host.navigate).toHaveBeenCalledWith('/p/p1/board');
  });

  it('ist rund: Ring und Kreis-Ausschnitt, „N“ am Rand, Karte nur Schmuck', () => {
    renderWithGame(<SaRadar />);
    const svg = radar().querySelector('svg')!;
    expect(svg.getAttribute('aria-hidden')).toBe('true');
    expect(svg.querySelector('clipPath circle')).not.toBeNull();
    expect(svg.querySelector('.gta-sa-radar-ring')).not.toBeNull();
    expect(svg.querySelector('text')?.textContent).toBe('N');
  });

  it('zeigt je offenem Ticket einen Blip in der Farbe seiner Spalte an seiner Position im Kreis', () => {
    const game = fakeGame();
    renderWithGame(<SaRadar />, game);
    expect(blips().map((blip) => blip.getAttribute('data-blip'))).toEqual(game.blips.map((b) => b.id));
    for (const blip of game.blips) {
      const el = radar().querySelector(`[data-blip="${blip.id}"]`) as SVGElement;
      expect(el.getAttribute('fill')).toBe(`var(--gta-c-${blip.column})`);
      const cx = Number(el.getAttribute('x')) + Number(el.getAttribute('width')) / 2;
      const cy = Number(el.getAttribute('y')) + Number(el.getAttribute('height')) / 2;
      expect(cx).toBeCloseTo(80 + (blip.x - 0.5) * RADAR_INNER, 1);
      expect(cy).toBeCloseTo(80 + (blip.y - 0.5) * RADAR_INNER, 1);
    }
  });

  it('nur blockierte Blips blinken', () => {
    renderWithGame(<SaRadar />);
    expect(blips().filter((blip) => blip.hasAttribute('data-blink')).map((blip) => blip.getAttribute('data-blip'))).toEqual(['NOVA-0004']);
  });

  it('nennt die Einzahl richtig und hat ohne Tickets keine Blips', () => {
    renderWithGame(<SaRadar />, fakeGame({ open: 1, blips: fakeGame().blips.slice(0, 1) }));
    expect(screen.getByRole('link', { name: 'Radar: 1 offenes Ticket, zum Board' })).toBeTruthy();
    cleanup();
    renderWithGame(<SaRadar />, fakeGame({ open: 0, blips: [] }));
    expect(blips()).toHaveLength(0);
  });
});
