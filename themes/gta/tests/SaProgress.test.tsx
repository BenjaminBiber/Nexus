// @vitest-environment jsdom
// Fortschritt im Stil San Andreas: durchgehender Balken mit schwarzem Rand wie die HUD-Balken, darunter „2/4 Schritte“
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { SaProgress } from '../src/sa/SaProgress.js';

afterEach(cleanup);

const bar = () => screen.getByRole('progressbar');
const fill = () => bar().querySelector('.gta-sa-progress-bar') as HTMLElement;

describe('SaProgress', () => {
  it('ist ein Fortschrittsbalken mit Bezeichnung und Werten', () => {
    render(<SaProgress done={2} total={4} label="Fortschritt der Schritte" caption="Schritte" />);
    expect(bar().getAttribute('aria-label')).toBe('Fortschritt der Schritte');
    expect(bar().getAttribute('aria-valuemin')).toBe('0');
    expect(bar().getAttribute('aria-valuemax')).toBe('4');
    expect(bar().getAttribute('aria-valuenow')).toBe('2');
    expect(bar().querySelector('.gta-sa-progress-text')?.textContent).toBe('2/4 Schritte');
  });

  it('ein durchgehender Balken, gefüllt im Anteil der erledigten Schritte', () => {
    render(<SaProgress done={1} total={3} label="Schritte" caption="Schritte" />);
    expect(fill().getAttribute('aria-hidden')).toBe('true');
    expect(fill().style.getPropertyValue('--sa-p')).toBe('33.3%');
    expect(bar().querySelectorAll('.gta-sa-progress-bar i')).toHaveLength(0);
  });

  it('nimmt unsinnige Werte hin: begrenzt, ganzzahlig, ohne Schritte leer', () => {
    render(<SaProgress done={9} total={3.7} label="Schritte" caption="Schritte" />);
    expect(bar().getAttribute('aria-valuenow')).toBe('3');
    expect(bar().getAttribute('aria-valuemax')).toBe('3');
    expect(bar().hasAttribute('data-complete')).toBe(true);
    expect(fill().style.getPropertyValue('--sa-p')).toBe('100%');
    cleanup();
    render(<SaProgress done={Number.NaN} total={0} label="Schritte" caption="Schritte" />);
    expect(bar().getAttribute('aria-valuenow')).toBe('0');
    expect(fill().style.getPropertyValue('--sa-p')).toBe('0%');
    expect(bar().hasAttribute('data-complete')).toBe(false);
  });
});
