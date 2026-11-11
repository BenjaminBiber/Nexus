// @vitest-environment jsdom
// Handy-Nachricht bei neuen Fragen: Ansehen führt zu den Fragen, Schließen blendet sie aus
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, screen } from '@testing-library/react';
import { VPhone } from '../src/v/VPhone.js';
import { fakeGame, renderWithGame } from './helpers/fakes.js';

afterEach(cleanup);

const phone = () => document.querySelector('[data-gta-part="phone"]') as HTMLElement | null;

describe('VPhone', () => {
  it('ohne Anruf rendert nichts', () => {
    renderWithGame(<VPhone />);
    expect(phone()).toBeNull();
  });

  it('meldet eine neue Frage in der Einzahl, mehrere in der Mehrzahl', () => {
    renderWithGame(<VPhone />, fakeGame({ phone: { count: 1 } }));
    expect(phone()?.textContent).toContain('Neue Frage');
    cleanup();
    renderWithGame(<VPhone />, fakeGame({ phone: { count: 3 } }));
    expect(phone()?.textContent).toContain('3 neue Fragen');
  });

  it('zeigt das gelbe Fragezeichen als Schmuck', () => {
    renderWithGame(<VPhone />, fakeGame({ phone: { count: 1 } }));
    const icon = phone()!.querySelector('.gta-v-phone-icon') as HTMLElement;
    expect(icon.getAttribute('aria-hidden')).toBe('true');
    expect(icon.querySelector('svg')).not.toBeNull();
  });

  it('„Ansehen“ ruft answerPhone, „Schließen“ dismissPhone', () => {
    const game = fakeGame({ phone: { count: 2 } });
    renderWithGame(<VPhone />, game);
    fireEvent.click(screen.getByRole('button', { name: 'Ansehen' }));
    expect(game.answerPhone).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Nachricht schließen' }));
    expect(game.dismissPhone).toHaveBeenCalledTimes(1);
  });
});
