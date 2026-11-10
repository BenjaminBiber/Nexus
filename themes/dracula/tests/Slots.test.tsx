// @vitest-environment jsdom
// Mermaid-Aussehen und Slot Progress von „Dracula“ und „Alucard“ (gemeinsamer Code, siehe package.test.ts)
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import draculaTheme from '../src/main.js';
import alucardTheme from '../../alucard/src/main.js';
import { DRACULA } from '../src/palette.js';
import { ALUCARD } from '../../alucard/src/palette.js';
import { DraculaProgress } from '../src/Progress.js';

const THEMES = [
  { id: 'dracula', name: 'Dracula', dark: true, definition: draculaTheme, palette: DRACULA },
  { id: 'alucard', name: 'Alucard', dark: false, definition: alucardTheme, palette: ALUCARD },
] as const;

// Kontrast nach WCAG 2.x
function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

afterEach(() => {
  cleanup();
  for (const key of Object.keys(DRACULA)) document.documentElement.style.removeProperty(`--dracula-${key}`);
});

describe.each(THEMES)('Theme „$name“', ({ id, dark, definition, palette }) => {
  it('meldet die id und den Slot Progress', () => {
    expect(definition.id).toBe(id);
    expect(Object.keys(definition.slots)).toEqual(['Progress']);
  });

  it('zeichnet Mermaid ohne gesetzte Variablen in den Farben aus palette.ts', () => {
    const look = definition.mermaid!();
    expect(look.theme).toBe('base');
    expect(look.themeVariables).toMatchObject({
      darkMode: dark,
      background: palette.bg,
      primaryColor: palette.node,
      primaryTextColor: palette.fg,
      primaryBorderColor: palette.purple,
      secondaryBorderColor: palette.pink,
      lineColor: palette.cyan,
      clusterBorder: palette.comment,
      noteBkgColor: palette.note,
      noteBorderColor: palette.yellow,
      actorBorder: palette.purple,
      pie1: palette.purple,
      pie2: palette.pink,
      git0: palette.purple,
      critBkgColor: palette.red,
      doneTaskBkgColor: palette.green,
    });
    expect(look.themeCSS).toContain('.node rect');
    expect(look.themeCSS).toContain(palette.cyan);
  });

  it('übernimmt die --dracula-* Farben aus theme.css', () => {
    document.documentElement.style.setProperty('--dracula-purple', '#123456');
    document.documentElement.style.setProperty('--dracula-cyan', '#654321');
    const look = definition.mermaid!();
    expect(look.themeVariables?.nodeBorder).toBe('#123456');
    expect(look.themeVariables?.lineColor).toBe('#654321');
    expect(look.themeCSS).toContain('#654321');
    expect(look.themeCSS).not.toContain(palette.cyan);
  });

  it('macht Schrift in Knoten und Notizen lesbar (4,5:1), auf Tortenstücken und Gantt-Balken 3:1', () => {
    const v = definition.mermaid!().themeVariables as Record<string, string>;
    expect(contrast(v.primaryTextColor, v.primaryColor)).toBeGreaterThanOrEqual(4.5);
    expect(contrast(v.noteTextColor, v.noteBkgColor)).toBeGreaterThanOrEqual(4.5);
    expect(contrast(v.textColor, v.background)).toBeGreaterThanOrEqual(4.5);
    for (let i = 1; i <= 7; i++) expect(contrast(v.pieSectionTextColor, v[`pie${i}`]), `pie${i}`).toBeGreaterThanOrEqual(3);
    expect(contrast(v.taskTextColor, v.taskBkgColor)).toBeGreaterThanOrEqual(3);
    expect(contrast(v.lineColor, v.background)).toBeGreaterThanOrEqual(3);
  });
});

describe('Slot Progress', () => {
  const value = (container: HTMLElement) => container.querySelector('.progress-ring-value') as SVGCircleElement;
  const offset = (container: HTMLElement) => Number(value(container).getAttribute('stroke-dashoffset'));
  const full = 2 * Math.PI * 34;

  it('zeigt den Ring wie der Standard mit Prozent, Zähler und Beschriftung', () => {
    const { container } = render(<DraculaProgress done={3} total={4} label="Fortschritt der Schritte" caption="Schritte erledigt" />);
    expect(screen.getByRole('img', { name: 'Fortschritt der Schritte: 75 %' })).toBeTruthy();
    expect(container.querySelector('.progress-ring-percent')?.textContent).toBe('75%');
    expect(container.querySelector('.progress-ring-count')?.textContent).toBe('3 / 4');
    expect(container.querySelector('.progress-ring-text .muted')?.textContent).toBe('Schritte erledigt');
    expect(offset(container)).toBeCloseTo(full * 0.25);
  });

  it('färbt den Bogen mit einem eigenen Verlauf von Purple nach Pink', () => {
    const { container } = render(
      <>
        <DraculaProgress done={1} total={2} label="a" caption="" />
        <DraculaProgress done={1} total={2} label="b" caption="" />
      </>,
    );
    const ids = [...container.querySelectorAll('linearGradient')].map((g) => g.id);
    expect(new Set(ids).size).toBe(2);
    expect(value(container).style.stroke).toBe(`url(#${ids[0]})`);
    const stops = [...container.querySelectorAll(`#${ids[0]} stop`)].map((s) => (s as SVGStopElement).style.stopColor);
    expect(stops).toEqual(['var(--dracula-purple)', 'var(--dracula-pink)']);
  });

  it('bleibt bei 0 von 0 leer und läuft nie über', () => {
    const { container, rerender } = render(<DraculaProgress done={0} total={0} label="p" caption="" />);
    expect(offset(container)).toBeCloseTo(full);
    rerender(<DraculaProgress done={12} total={10} label="p" caption="" />);
    expect(offset(container)).toBeCloseTo(0);
    expect(container.querySelector('.progress-ring-percent')?.textContent).toBe('100%');
  });
});
