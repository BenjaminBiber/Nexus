// @vitest-environment jsdom
// Wortmarken: fertig umrissene Pfade als SVG in der Textfarbe; mit Bezeichnung ein Bild, sonst Schmuck
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { Wordmark } from '../src/shared/Wordmark.js';
import { WORDMARKS, type WordmarkName } from '../src/shared/wordmarks.js';

afterEach(cleanup);

const NAMES: WordmarkName[] = ['nexus', 'missionPassed', 'missionPassedSa', 'respect', 'wasted', 'busted', 'alert'];

describe('Wortmarken', () => {
  it('kennt genau die Marken beider Previews, gleiche Einträge nur einmal', () => {
    expect(Object.keys(WORDMARKS).sort()).toEqual([...NAMES].sort());
    expect(WORDMARKS.nexus.box).toEqual([2, -64, 253, 64]);
    expect(WORDMARKS.missionPassed.box).toEqual([2, -80, 663, 100]);
    expect(WORDMARKS.missionPassedSa.box).toEqual([2, -80, 690, 100]);
    expect(WORDMARKS.nexus.d.startsWith('M49.3 0L49.3 -55.8')).toBe(true);
  });

  it.each(NAMES)('%s rendert den Pfad mit viewBox samt Rand', (name) => {
    const { container } = render(<Wordmark name={name} />);
    const svg = container.querySelector('svg')!;
    const [x, y, w, h] = WORDMARKS[name].box;
    expect(svg.getAttribute('viewBox')).toBe(`${x - 9} ${y - 9} ${w + 18} ${h + 18}`);
    expect(svg.querySelector('path')?.getAttribute('d')).toBe(WORDMARKS[name].d);
    expect(svg.getAttribute('fill')).toBe('currentColor');
    expect(svg.getAttribute('class')).toBe('gta-wordmark');
    expect(svg.getAttribute('data-wordmark')).toBe(name);
  });

  it('ohne Bezeichnung ist sie für Screenreader versteckt', () => {
    const { container } = render(<Wordmark name="wasted" />);
    const svg = container.querySelector('svg')!;
    expect(svg.getAttribute('aria-hidden')).toBe('true');
    expect(svg.hasAttribute('role')).toBe(false);
    expect(svg.hasAttribute('aria-label')).toBe(false);
  });

  it('mit Bezeichnung ist sie ein Bild mit Namen und behält eigene Klassen', () => {
    render(<Wordmark name="nexus" label="Nexus" className="gta-logo" />);
    const svg = screen.getByRole('img', { name: 'Nexus' });
    expect(svg.hasAttribute('aria-hidden')).toBe(false);
    expect(svg.getAttribute('class')).toBe('gta-wordmark gta-logo');
  });
});
