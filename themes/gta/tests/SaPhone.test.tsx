// @vitest-environment jsdom
// Handy-Anruf bei neuen Fragen: das klingelnde Handy mit Anzeige, Annehmen (Y) und Ablehnen (N)
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, screen } from '@testing-library/react';
import { SaPhone } from '../src/sa/SaPhone.js';
import { fakeGame, renderWithGame } from './helpers/fakes.js';

afterEach(cleanup);

const phone = () => document.querySelector('[data-gta-part="phone"]') as HTMLElement | null;

describe('SaPhone', () => {
  it('ohne Anruf rendert nichts', () => {
    renderWithGame(<SaPhone />);
    expect(phone()).toBeNull();
  });

  it('zeigt auf dem Display eine neue Frage in der Einzahl, mehrere in der Mehrzahl', () => {
    renderWithGame(<SaPhone />, fakeGame({ phone: { count: 1 } }));
    expect(phone()!.querySelector('.gta-sa-phone-display')?.textContent).toBe('Neue Frage');
    expect(phone()!.getAttribute('aria-label')).toBe('Anruf: Neue Frage');
    cleanup();
    renderWithGame(<SaPhone />, fakeGame({ phone: { count: 3 } }));
    expect(phone()!.querySelector('.gta-sa-phone-display')?.textContent).toBe('3 neue Fragen');
  });

  it('das Handy ist selbst gezeichnet und nur Schmuck', () => {
    renderWithGame(<SaPhone />, fakeGame({ phone: { count: 1 } }));
    const cell = phone()!.querySelector('svg.gta-sa-cell') as SVGElement;
    expect(cell.getAttribute('aria-hidden')).toBe('true');
  });

  it('Annehmen ruft answerPhone, Ablehnen dismissPhone; die Tasten Y und N stehen daneben', () => {
    const game = fakeGame({ phone: { count: 2 } });
    renderWithGame(<SaPhone />, game);
    const answer = screen.getByRole('button', { name: /Annehmen/ });
    const decline = screen.getByRole('button', { name: /Ablehnen/ });
    expect(answer.querySelector('kbd')?.textContent).toBe('Y');
    expect(decline.querySelector('kbd')?.textContent).toBe('N');
    fireEvent.click(answer);
    expect(game.answerPhone).toHaveBeenCalledTimes(1);
    fireEvent.click(decline);
    expect(game.dismissPhone).toHaveBeenCalledTimes(1);
  });
});
