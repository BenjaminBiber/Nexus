// @vitest-environment jsdom
// Stats-Menü: Dialog im Aufbau der Dashboard-Seite der Preview mit Kategorien, Zeilen und 100-%-Checkliste aus echten Daten
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { ThemeHostProvider } from '@nexus/theme-sdk';
import { GameContext, type Game } from '../src/data/GameContext.js';
import { VShell } from '../src/v/VShell.js';
import { VStats } from '../src/v/VStats.js';
import { fakeGame, fakeHost, renderWithGame } from './helpers/fakes.js';

afterEach(cleanup);

const dialog = () => screen.getByRole('dialog', { name: 'Stats' });
const rows = () =>
  Object.fromEntries(
    [...dialog().querySelectorAll('.gta-v-stats-rows .gta-v-row')].map((row) => [
      row.querySelector('.gta-v-row-label')?.textContent,
      row.querySelector('.gta-v-row-value')?.textContent ?? '',
    ]),
  );
const open = (over: Partial<Game> = {}) => renderWithGame(<VStats />, fakeGame({ statsOpen: true, ...over }));

describe('VStats', () => {
  it('ist ein modaler Dialog „Stats“ mit Prüfmarke und holt den Fokus hinein', () => {
    open();
    const stats = dialog();
    expect(stats.getAttribute('aria-modal')).toBe('true');
    expect(stats.getAttribute('data-gta-part')).toBe('stats');
    expect(stats.contains(document.activeElement)).toBe(true);
  });

  it('„Allgemein“ zeigt die Kennzahlen des Boards', () => {
    open({ wanted: 1 });
    expect(rows()).toEqual({
      'Tickets gesamt': '7',
      Abgeschlossen: '2',
      Offen: '5',
      Blockiert: '1',
      'Fortschritt des Boards': '29 %',
      'Schritte erledigt': '3 / 12',
      'Offene Fragen': '1',
      Fahndungslevel: '★Stufe 1 von 5 (1 blockiert)',
      'Verdient insgesamt': '$87.500',
    });
  });

  it('„Spalten“ zeigt je Spalte Anzahl und Balken in der Spaltenfarbe', () => {
    open();
    fireEvent.click(within(dialog()).getByRole('button', { name: 'Spalten' }));
    expect(within(dialog()).getByRole('button', { name: 'Spalten' }).getAttribute('aria-pressed')).toBe('true');
    expect(rows()).toEqual({ Backlog: '1', Wartet: '1', 'In Planung': '0', Geplant: '1', 'In Arbeit': '1', Blockiert: '1', Erledigt: '2' });
    const bar = dialog().querySelector('.gta-v-stats-rows .gta-v-row .gta-v-statbar') as HTMLElement;
    expect(bar.style.getPropertyValue('--v-c')).toBe('var(--gta-c-backlog)');
  });

  it('„Claude-Verbrauch“ zeigt den Rest beider Fenster; ohne Werte einen Hinweis', () => {
    open({ quota5h: 0.58, quota7d: 0.82 });
    fireEvent.click(within(dialog()).getByRole('button', { name: 'Claude-Verbrauch' }));
    expect(rows()).toEqual({ 'Rest 5 Stunden': '58 %', 'Rest 7 Tage': '82 %' });
    cleanup();
    open({ quota5h: null, quota7d: null });
    fireEvent.click(within(dialog()).getByRole('button', { name: 'Claude-Verbrauch' }));
    expect(rows()).toEqual({});
    expect(dialog().textContent).toContain('Claude-Verbrauch ist gerade nicht verfügbar');
  });

  it('rechts die 100-%-Checkliste mit Prozent, Anzahl und den nächsten drei Tickets', () => {
    open();
    const list = dialog().querySelector('.gta-v-checklist') as HTMLElement;
    expect(list.querySelector('.gta-v-pct')?.textContent).toBe('29 %2 von 7 Tickets');
    const next = [...list.querySelectorAll('li')].map((li) => li.textContent);
    expect(next).toEqual(['NOVA-0004 Zahlungsanbieter', 'NOVA-0003 Kasse', 'NOVA-0005 Gutscheine']);
  });

  it('Esc, der Schließen-Knopf und ein Klick daneben schließen das Menü', () => {
    const game = fakeGame({ statsOpen: true });
    renderWithGame(<VStats />, game);
    fireEvent.keyDown(dialog(), { key: 'Escape' });
    expect(game.setStatsOpen).toHaveBeenLastCalledWith(false);
    fireEvent.click(within(dialog()).getByRole('button', { name: 'Stats schließen' }));
    expect(game.setStatsOpen).toHaveBeenCalledTimes(2);
    fireEvent.click(document.querySelector('.gta-v-stats-backdrop')!);
    expect(game.setStatsOpen).toHaveBeenCalledTimes(3);
  });

  it('die Sterne sind für Screenreader die Stufe', () => {
    open({ wanted: 2 });
    const value = [...dialog().querySelectorAll('.gta-v-row')].find((row) => row.textContent?.startsWith('Fahndungslevel'))!;
    expect(value.querySelector('[aria-hidden="true"]')?.textContent).toBe('★★');
    expect(value.querySelector('.gta-sr')?.textContent).toBe('Stufe 2 von 5');
  });

  it('ein Seitenwechsel (etwa mit Q oder E) schließt das Menü', () => {
    const game = fakeGame({ statsOpen: true });
    const host = fakeHost();
    const wrap = (g: Game): ReactElement => (
      <ThemeHostProvider value={host}>
        <GameContext.Provider value={g}>
          <VStats />
        </GameContext.Provider>
      </ThemeHostProvider>
    );
    const view = render(wrap(game));
    view.rerender(wrap({ ...game, now: new Date(2026, 10, 11, 9, 42) }));
    expect(game.setStatsOpen).not.toHaveBeenCalled();
    const docs = game.nav.find((item) => item.id === 'docs');
    view.rerender(wrap({ ...game, active: docs }));
    expect(game.setStatsOpen).toHaveBeenCalledWith(false);
  });

  it('Tab bleibt im Dialog: vom letzten zum ersten Element und zurück', () => {
    open();
    const focusable = [...dialog().querySelectorAll<HTMLElement>('button')];
    focusable.at(-1)!.focus();
    fireEvent.keyDown(dialog(), { key: 'Tab' });
    expect(document.activeElement).toBe(focusable[0]);
    fireEvent.keyDown(dialog(), { key: 'Tab', shiftKey: true });
    expect(document.activeElement).toBe(focusable.at(-1));
  });

  it('im Rahmen: öffnet über den Stats-Knopf und gibt den Fokus danach dorthin zurück', () => {
    const host = fakeHost();
    const wrap = (game: Game): ReactElement => (
      <ThemeHostProvider value={host}>
        <GameContext.Provider value={game}>
          <VShell content={null} />
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
