// @vitest-environment jsdom
// Fortschritt als Segmentbalken wie auf den Karten der Preview („2/4 SCHRITTE“)
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { MAX_SEGMENTS, VProgress } from '../src/v/VProgress.js';

afterEach(cleanup);

const bar = () => screen.getByRole('progressbar');
const segments = () => [...bar().querySelectorAll('.gta-v-progress-bar i')];

describe('VProgress', () => {
  it('ist ein Fortschrittsbalken mit Bezeichnung und Werten', () => {
    render(<VProgress done={2} total={4} label="Fortschritt der Schritte" caption="Schritte" />);
    expect(bar().getAttribute('aria-label')).toBe('Fortschritt der Schritte');
    expect(bar().getAttribute('aria-valuemin')).toBe('0');
    expect(bar().getAttribute('aria-valuemax')).toBe('4');
    expect(bar().getAttribute('aria-valuenow')).toBe('2');
    expect(bar().querySelector('.gta-v-progress-text')?.textContent).toBe('2/4 Schritte');
  });

  it('ein Segment je Schritt, die erledigten gefüllt', () => {
    render(<VProgress done={2} total={4} label="Schritte" caption="Schritte" />);
    expect(segments().map((s) => s.hasAttribute('data-on'))).toEqual([true, true, false, false]);
    expect(bar().querySelector('.gta-v-progress-bar')?.getAttribute('aria-hidden')).toBe('true');
  });

  it('über MAX_SEGMENTS Schritten ein durchgehender Balken in Prozent', () => {
    render(<VProgress done={5} total={MAX_SEGMENTS + 8} label="Schritte" caption="Schritte" />);
    expect(segments()).toHaveLength(1);
    expect((segments()[0] as HTMLElement).style.width).toBe('25%');
  });

  it('nimmt unsinnige Werte hin: begrenzt, ganzzahlig, ohne Schritte ein leerer Balken', () => {
    render(<VProgress done={9} total={3.7} label="Schritte" caption="Schritte" />);
    expect(bar().getAttribute('aria-valuenow')).toBe('3');
    expect(bar().getAttribute('aria-valuemax')).toBe('3');
    expect(bar().hasAttribute('data-complete')).toBe(true);
    cleanup();
    render(<VProgress done={Number.NaN} total={0} label="Schritte" caption="Schritte" />);
    expect(bar().getAttribute('aria-valuenow')).toBe('0');
    expect(segments()).toHaveLength(1);
    expect(segments()[0]!.hasAttribute('data-on')).toBe(false);
    expect(bar().hasAttribute('data-complete')).toBe(false);
  });
});
