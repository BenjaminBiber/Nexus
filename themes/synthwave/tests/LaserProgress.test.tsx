// @vitest-environment jsdom
// Fortschritt als Laserbalken (Slot Progress): ein Segment je Schritt bis 12, darüber ein durchgehender Balken
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { LaserProgress } from '../src/LaserProgress.js';

afterEach(cleanup);

const LABEL = 'Fortschritt der Schritte';
const CAPTION = 'Schritte erledigt';

describe('LaserProgress', () => {
  it('3/5: fünf Segmente, drei davon leuchten; Fortschrittsbalken mit Werten und Text', () => {
    const { container } = render(<LaserProgress done={3} total={5} label={LABEL} caption={CAPTION} />);
    const bar = screen.getByRole('progressbar', { name: LABEL });
    expect(bar.getAttribute('aria-valuemin')).toBe('0');
    expect(bar.getAttribute('aria-valuenow')).toBe('3');
    expect(bar.getAttribute('aria-valuemax')).toBe('5');
    const segments = container.querySelectorAll('.synth-laser > i');
    expect(segments).toHaveLength(5);
    expect(container.querySelectorAll('.synth-laser > i[data-on]')).toHaveLength(3);
    expect(container.querySelector('.synth-laser-beam')).toBeNull();
    expect(container.querySelector('.synth-progress-text')?.textContent).toBe('3/5 Schritte erledigt');
    expect(bar.hasAttribute('data-complete')).toBe(false);
  });

  it('20/40: kein Segment-Raster, ein durchgehender Balken mit 50 % Breite', () => {
    const { container } = render(<LaserProgress done={20} total={40} label={LABEL} caption={CAPTION} />);
    const laser = container.querySelector('.synth-laser');
    expect(laser?.classList.contains('synth-laser-beam')).toBe(true);
    expect(laser?.querySelectorAll('i')).toHaveLength(1);
    expect((laser?.querySelector('i') as HTMLElement).style.width).toBe('50%');
    expect(container.querySelector('.synth-progress-text')?.textContent).toBe('20/40 Schritte erledigt');
  });

  it('12 Schritte sind noch Segmente, 13 schon ein durchgehender Balken', () => {
    const twelve = render(<LaserProgress done={6} total={12} label={LABEL} caption={CAPTION} />);
    expect(twelve.container.querySelectorAll('.synth-laser > i')).toHaveLength(12);
    expect(twelve.container.querySelector('.synth-laser-beam')).toBeNull();
    cleanup();
    const thirteen = render(<LaserProgress done={6} total={13} label={LABEL} caption={CAPTION} />);
    expect(thirteen.container.querySelector('.synth-laser-beam')).not.toBeNull();
  });

  it('0/0: rendert ohne Fehler, ein gedämpftes Segment, kein NaN im Markup', () => {
    const { container } = render(<LaserProgress done={0} total={0} label={LABEL} caption={CAPTION} />);
    expect(container.innerHTML).not.toContain('NaN');
    expect(container.querySelectorAll('.synth-laser > i')).toHaveLength(1);
    expect(container.querySelectorAll('.synth-laser > i[data-on]')).toHaveLength(0);
    const bar = screen.getByRole('progressbar');
    expect(bar.getAttribute('aria-valuenow')).toBe('0');
    expect(bar.getAttribute('aria-valuemax')).toBe('0');
    expect(bar.hasAttribute('data-complete')).toBe(false);
  });

  it('5/5: data-complete ist gesetzt, alle Segmente leuchten', () => {
    const { container } = render(<LaserProgress done={5} total={5} label={LABEL} caption={CAPTION} />);
    expect(screen.getByRole('progressbar').hasAttribute('data-complete')).toBe(true);
    expect(container.querySelectorAll('.synth-laser > i[data-on]')).toHaveLength(5);
  });

  it('40/40: der durchgehende Balken ist voll und data-complete gesetzt', () => {
    const { container } = render(<LaserProgress done={40} total={40} label={LABEL} caption={CAPTION} />);
    expect(screen.getByRole('progressbar').hasAttribute('data-complete')).toBe(true);
    expect((container.querySelector('.synth-laser-beam i') as HTMLElement).style.width).toBe('100%');
  });

  it('7/5 wird auf 5 begrenzt, negative Werte auf 0', () => {
    const over = render(<LaserProgress done={7} total={5} label={LABEL} caption={CAPTION} />);
    expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('5');
    expect(over.container.querySelector('.synth-progress-text')?.textContent).toBe('5/5 Schritte erledigt');
    cleanup();
    const under = render(<LaserProgress done={-2} total={-1} label={LABEL} caption={CAPTION} />);
    const bar = screen.getByRole('progressbar');
    expect(bar.getAttribute('aria-valuenow')).toBe('0');
    expect(bar.getAttribute('aria-valuemax')).toBe('0');
    expect(under.container.innerHTML).not.toContain('NaN');
  });
});
