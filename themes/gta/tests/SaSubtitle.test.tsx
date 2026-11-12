// @vitest-environment jsdom
// Untertitel-Zeile unten mittig: Cheat-Meldungen und der Spruch nach einem abgelehnten Anruf
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup } from '@testing-library/react';
import { SaSubtitle } from '../src/sa/SaSubtitle.js';
import { fakeGame, renderWithGame } from './helpers/fakes.js';

afterEach(cleanup);

const live = () => document.querySelector('.gta-sa-sub') as HTMLElement;
const line = () => live().querySelector('.gta-sa-sub-line');
const cheat = () => document.querySelector('[data-gta-part="cheat"]') as HTMLElement | null;

describe('SaSubtitle', () => {
  it('ohne Anlass steht nur die leere Hülle, die Änderungen höflich ansagt', () => {
    renderWithGame(<SaSubtitle />);
    expect(live().getAttribute('aria-live')).toBe('polite');
    expect(line()).toBeNull();
    expect(cheat()).toBeNull();
  });

  it.each([
    ['money', 'Cheat aktiviert – +$250000'],
    ['clear', 'Cheat aktiviert – Fahndungslevel aufgehoben'],
    ['wanted', 'Cheat aktiviert – Sechs Sterne'],
  ] as const)('Cheat %s: Meldung in der Zeile', (id, text) => {
    renderWithGame(<SaSubtitle />, fakeGame({ cheat: id }));
    expect(cheat()?.textContent).toBe(text);
    expect(live().contains(cheat())).toBe(true);
  });

  it('nach einem abgelehnten Anruf ein Spruch mit dem Hinweis auf die Fragen', () => {
    renderWithGame(<SaSubtitle />, fakeGame({ declined: true }));
    expect(line()?.textContent).toBe('Du: Später. Die Frage wartet unter „Fragen“.');
    expect(cheat()).toBeNull();
  });

  it('ein Cheat geht dem Spruch vor', () => {
    renderWithGame(<SaSubtitle />, fakeGame({ declined: true, cheat: 'clear' }));
    expect(line()?.textContent).toBe('Cheat aktiviert – Fahndungslevel aufgehoben');
  });
});
