// @vitest-environment jsdom
// Stats-Menü im Stil San Andreas: Dialog mit Statistik, Revierkarte und Zahlen aus echten Daten
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { ThemeHostProvider } from '@nexus/theme-sdk';
import { GameContext, type Game } from '../src/data/GameContext.js';
import { SaShell } from '../src/sa/SaShell.js';
import { SaStats } from '../src/sa/SaStats.js';
import { fakeGame, fakeHost, renderWithGame } from './helpers/fakes.js';

afterEach(cleanup);

const dialog = () => screen.getByRole('dialog', { name: 'Stats' });
const open = (over: Partial<Game> = {}) => renderWithGame(<SaStats />, fakeGame({ statsOpen: true, ...over }));
const panel = (title: string) => within(dialog()).getByRole('heading', { name: title }).closest('section') as HTMLElement;
const stats = () =>
  [...panel('Statistik').querySelectorAll('.gta-sa-stat')].map((stat) => [
    stat.querySelector('.gta-sa-stat-name')?.textContent,
    stat.querySelector('.gta-sa-stat-value')?.textContent,
    (stat.querySelector('.gta-sa-sbar') as HTMLElement).style.getPropertyValue('--sa-p'),
  ]);
const numbers = () =>
  Object.fromEntries([...panel('Zahlen').querySelectorAll('.gta-sa-st')].map((row) => [row.querySelector('span')?.textContent, row.querySelector('b')?.textContent]));

describe('SaStats', () => {
  it('ist ein modaler Dialog „Stats“ mit Prüfmarke und holt den Fokus hinein', () => {
    open();
    expect(dialog().getAttribute('aria-modal')).toBe('true');
    expect(dialog().getAttribute('data-gta-part')).toBe('stats');
    expect(dialog().contains(document.activeElement)).toBe(true);
  });

  it('„Statistik“: Respekt, Ausdauer, Panzerung und Schritte als Balken mit Zahl', () => {
    open({ quota5h: 0.62, quota7d: 0.81 });
    expect(stats()).toEqual([
      ['Respekt', '29 %', '29%'],
      ['Ausdauer', '62 % frei', '62%'],
      ['Panzerung', '81 % frei', '81%'],
      ['Schritte', '3/12', '25%'],
    ]);
  });

  it('„Statistik“ ohne Werte: Striche und leere Balken', () => {
    open({ boardProgress: null, quota5h: null, quota7d: null, stepsDone: 0, stepsTotal: 0 });
    expect(stats()).toEqual([
      ['Respekt', '–', '0%'],
      ['Ausdauer', '–', '0%'],
      ['Panzerung', '–', '0%'],
      ['Schritte', '0/0', '0%'],
    ]);
  });

  it('„Reviere“: je Spalte ein Gebiet in der Spaltenfarbe, leere gedimmt, Legende mit Titel und Anzahl', () => {
    const game = fakeGame({ statsOpen: true });
    renderWithGame(<SaStats />, game);
    const turfs = [...panel('Reviere').querySelectorAll<SVGElement>('[data-turf]')];
    expect(turfs.map((turf) => turf.getAttribute('data-turf'))).toEqual(game.columns.map((column) => column.id));
    for (const turf of turfs) expect(turf.getAttribute('fill')).toBe(`var(--gta-c-${turf.getAttribute('data-turf')})`);
    expect(turfs.filter((turf) => turf.hasAttribute('data-empty')).map((turf) => turf.getAttribute('data-turf'))).toEqual(['in_planning']);
    expect([...panel('Reviere').querySelectorAll('.gta-sa-legend li')].map((li) => li.textContent)).toEqual([
      'Backlog 1',
      'Wartet 1',
      'In Planung 0',
      'Geplant 1',
      'In Arbeit 1',
      'Blockiert 1',
      'Erledigt 2',
    ]);
  });

  it('„Zahlen“: Kennzahlen des Boards, Fahndungslevel und Verdienst', () => {
    open({ wanted: 1 });
    expect(numbers()).toEqual({
      'Tickets gesamt': '7',
      Abgeschlossen: '2',
      Offen: '5',
      Blockiert: '1',
      'Offene Fragen': '1',
      Fahndungslevel: '1',
      'Verdient insgesamt': '$00087500',
    });
  });

  it('Esc und der Schließen-Knopf schließen das Menü', () => {
    const game = fakeGame({ statsOpen: true });
    renderWithGame(<SaStats />, game);
    fireEvent.keyDown(dialog(), { key: 'Escape' });
    expect(game.setStatsOpen).toHaveBeenLastCalledWith(false);
    fireEvent.click(within(dialog()).getByRole('button', { name: 'Stats schließen' }));
    expect(game.setStatsOpen).toHaveBeenCalledTimes(2);
  });

  it('ein Seitenwechsel schließt das Menü', () => {
    const game = fakeGame({ statsOpen: true });
    const host = fakeHost();
    const wrap = (g: Game): ReactElement => (
      <ThemeHostProvider value={host}>
        <GameContext.Provider value={g}>
          <SaStats />
        </GameContext.Provider>
      </ThemeHostProvider>
    );
    const view = render(wrap(game));
    view.rerender(wrap({ ...game, now: new Date(2026, 10, 11, 9, 42) }));
    expect(game.setStatsOpen).not.toHaveBeenCalled();
    view.rerender(wrap({ ...game, active: game.nav.find((item) => item.id === 'docs') }));
    expect(game.setStatsOpen).toHaveBeenCalledWith(false);
  });

  it('im Rahmen: öffnet über den Stats-Eintrag und gibt den Fokus danach dorthin zurück', () => {
    const host = fakeHost();
    const wrap = (game: Game): ReactElement => (
      <ThemeHostProvider value={host}>
        <GameContext.Provider value={game}>
          <SaShell content={null} />
        </GameContext.Provider>
      </ThemeHostProvider>
    );
    const game = fakeGame();
    const view = render(wrap(game));
    const button = document.querySelector('[data-gta-part="stats-button"]') as HTMLButtonElement;
    button.focus();
    view.rerender(wrap({ ...game, statsOpen: true }));
    expect(dialog().contains(document.activeElement)).toBe(true);
    view.rerender(wrap({ ...game, statsOpen: false }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(document.activeElement).toBe(button);
  });
});
