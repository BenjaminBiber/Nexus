// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { WIN95_ICONS } from '../src/components/win95/Win95Icons.js';
import { BoardIcon, CogIcon } from '../src/components/NavIcons.js';
import { ProgressRing } from '../src/components/ProgressRing.js';

afterEach(() => {
  cleanup();
  delete document.documentElement.dataset.theme;
});

describe('Pixel-Symbole „Windows 95“', () => {
  it('hat je Symbol höchstens 16 Zeilen zu je 16 Pixeln aus der Palette', () => {
    for (const [name, rows] of Object.entries(WIN95_ICONS)) {
      expect(rows.length, name).toBeLessThanOrEqual(16);
      for (const row of rows) expect(row, name).toMatch(/^[.kwsgyobnrmtcGp]{16}$/);
    }
  });

  it('zeigt im Schema „Windows 95“ das Pixel-Symbol statt des Linien-Symbols', () => {
    const { container, unmount } = render(<BoardIcon />);
    expect(container.querySelector('.w95-pixel-icon')).toBeNull();
    unmount();
    document.documentElement.dataset.theme = 'win95';
    const win95 = render(
      <>
        <BoardIcon />
        <CogIcon />
      </>,
    );
    expect(win95.container.querySelectorAll('svg.w95-pixel-icon')).toHaveLength(2);
    expect(win95.container.querySelector('.w95-px-y')).toBeTruthy();
  });
});

describe('Schrittanzeige „Windows 95“', () => {
  it('zeigt die Belegungs-Torte mit Erledigt, Offen und Gesamt', () => {
    document.documentElement.dataset.theme = 'win95';
    const { container } = render(<ProgressRing done={2} total={4} />);
    expect(screen.getByRole('img', { name: 'Fortschritt der Schritte: 50 %' })).toBeTruthy();
    expect(container.querySelector('.progress-ring')).toBeNull();
    expect(screen.getByText('50 % erledigt')).toBeTruthy();
    const values = [...container.querySelectorAll('.w95-pie-legend dd')].map((dd) => dd.textContent);
    expect(values).toEqual(['2', '2', '4']);
    expect(container.querySelector('path.w95-pie-done')).toBeTruthy();
  });

  it('füllt die Oberseite bei allen Schritten ganz und lässt sie ohne Schritte leer', () => {
    document.documentElement.dataset.theme = 'win95';
    const full = render(<ProgressRing done={3} total={3} />);
    expect(full.container.querySelector('ellipse.w95-pie-done')).toBeTruthy();
    full.unmount();
    const empty = render(<ProgressRing done={0} total={0} />);
    expect(empty.container.querySelector('.w95-pie-done')).toBeNull();
    expect(empty.container.querySelector('.w95-pie-done-side')).toBeNull();
  });
});
