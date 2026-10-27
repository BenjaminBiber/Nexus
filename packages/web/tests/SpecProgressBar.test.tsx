// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { SpecProgressBar, SpecProgressPie } from '../src/components/SpecProgressBar.js';

afterEach(cleanup);

const progress = (extra = {}) => ({ spec: '001-login', total: 8, done: 3, blocked: 1, waiting: 2, inProgress: 1, open: 1, notNeeded: 0, ...extra });

describe('SpecProgressBar', () => {
  it('zeigt Text und Segmente in Statusfarben mit Anteilen an der Summe', () => {
    const { container } = render(<SpecProgressBar progress={progress({ notNeeded: 2 })} />);
    expect(screen.getByText('3 von 8 erledigt, 1 blockiert, 2 wartet, 1 in Arbeit, 1 offen')).toBeTruthy();
    const parts = [...container.querySelectorAll<HTMLElement>('.spec-progress-bar > span')];
    expect(parts.map((p) => [p.dataset.status, p.style.width])).toEqual([
      ['done', '37.5%'], ['blocked', '12.5%'], ['waiting', '25%'], ['in_progress', '12.5%'], ['backlog', '12.5%'],
    ]);
    expect(screen.getByRole('img').getAttribute('aria-label')).toContain('(2 entfallen)');
  });

  it('lässt leere Gruppen weg und meldet eine Spec ohne Tickets', () => {
    const { container, rerender } = render(<SpecProgressBar progress={progress({ total: 2, done: 2, blocked: 0, waiting: 0, inProgress: 0, open: 0 })} />);
    expect(container.querySelectorAll('.spec-progress-bar > span')).toHaveLength(1);
    rerender(<SpecProgressBar progress={progress({ total: 0, done: 0, blocked: 0, waiting: 0, inProgress: 0, open: 0 })} />);
    expect(screen.getByText('keine Tickets')).toBeTruthy();
    expect(container.querySelectorAll('.spec-progress-bar > span')).toHaveLength(0);
  });
});

describe('SpecProgressPie', () => {
  it('teilt den Kreis in Segmente nach den Anteilen und nennt den Stand im Namen', () => {
    const { container } = render(<SpecProgressPie progress={progress()} />);
    const parts = [...container.querySelectorAll<SVGCircleElement>('circle[data-status]')];
    expect(parts.map((p) => p.dataset.status)).toEqual(['done', 'blocked', 'waiting', 'in_progress', 'backlog']);
    const lengths = parts.map((p) => Number(p.getAttribute('stroke-dasharray')!.split(' ')[0]));
    expect(lengths[0]! / lengths.reduce((a, b) => a + b, 0)).toBeCloseTo(0.375);
    // Jedes Segment beginnt, wo das vorige endet
    expect(Number(parts[1]!.getAttribute('stroke-dashoffset'))).toBeCloseTo(-lengths[0]!);
    expect(screen.getByRole('img', { name: 'Fortschritt 001-login: 3 von 8 erledigt, 1 blockiert, 2 wartet, 1 in Arbeit, 1 offen' })).toBeTruthy();
  });
});
