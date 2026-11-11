// @vitest-environment jsdom
// Menüspalte im Stil San Andreas: Wortmarke, Board-Auswahl, Menüliste aus useNav, Stats und Suche
import { createRef } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, screen, within } from '@testing-library/react';
import { SaMenu } from '../src/sa/SaMenu.js';
import { fakeGame, fakeHost, fakeNav, renderWithGame } from './helpers/fakes.js';

afterEach(cleanup);

const nav = () => screen.getByRole('navigation', { name: 'Hauptnavigation' });
const links = () => within(nav()).getAllByRole('link');

describe('SaMenu', () => {
  it('oben die Wortmarke und die Board-Auswahl des Kerns', () => {
    renderWithGame(<SaMenu />);
    expect(screen.getByRole('img', { name: 'Nexus' }).getAttribute('data-wordmark')).toBe('nexus');
    expect(screen.getByText('teil-board')).toBeTruthy();
  });

  it('ein Eintrag je Punkt der Navigation mit Symbol, Beschriftung und Zähler', () => {
    renderWithGame(<SaMenu />);
    expect(links().map((link) => link.querySelector('.gta-sa-menu-label')?.textContent)).toEqual([
      'Board',
      'Dashboard',
      'Meine Aufgaben',
      'Sessions',
      'Fragen',
      'Docs',
      'Anleitung',
      'Einstellungen',
    ]);
    expect(links().map((link) => link.querySelector('.gta-sa-menu-count')?.textContent ?? null)).toEqual([null, null, '2', null, '1', null, null, null]);
    for (const link of links()) expect(link.querySelector('svg.gta-sa-icon')?.getAttribute('aria-hidden')).toBe('true');
    expect(links()[0]!.getAttribute('href')).toBe('/p/p1/board');
  });

  it('der aktive Eintrag ist als aktuelle Seite markiert', () => {
    renderWithGame(<SaMenu />, fakeGame(), fakeHost({ nav: fakeNav('docs') }));
    expect(links().filter((link) => link.getAttribute('aria-current') === 'page').map((link) => link.textContent)).toEqual(['Docs']);
  });

  it('ein Klick wechselt die Seite über den Host', () => {
    const host = fakeHost();
    renderWithGame(<SaMenu />, fakeGame(), host);
    fireEvent.click(within(nav()).getByRole('link', { name: /Sessions/ }));
    expect(host.navigate).toHaveBeenCalledWith('/p/p1/sessions');
  });

  it('der Eintrag Stats öffnet das Stats-Menü und nimmt die übergebene Referenz', () => {
    const game = fakeGame();
    const ref = createRef<HTMLButtonElement>();
    renderWithGame(<SaMenu statsButton={ref} />, game);
    const stats = document.querySelector('[data-gta-part="stats-button"]') as HTMLButtonElement;
    expect(stats.getAttribute('aria-haspopup')).toBe('dialog');
    expect(ref.current).toBe(stats);
    fireEvent.click(stats);
    expect(game.setStatsOpen).toHaveBeenCalledWith(true);
  });

  it('die Suche öffnet sich per Klick, mit dem Hinweis Strg K', () => {
    const host = fakeHost();
    renderWithGame(<SaMenu />, fakeGame(), host);
    const search = screen.getByRole('button', { name: /Suchen/ });
    expect([...search.querySelectorAll('kbd')].map((kbd) => kbd.textContent)).toEqual(['Strg', 'K']);
    fireEvent.click(search);
    expect(host.openSearch).toHaveBeenCalledTimes(1);
  });
});
