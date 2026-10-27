// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { assignHeadingIds, slugify } from '../src/markdown/headingIds.js';

// Baut ein Wurzelelement aus HTML
function root(html: string): HTMLElement {
  const el = document.createElement('div');
  el.innerHTML = html;
  return el;
}

describe('slugify', () => {
  it('macht Kleinbuchstaben, behält Umlaute und ersetzt Sonderzeichen', () => {
    expect(slugify('Größe & Ziel: `x`')).toBe('größe-ziel-x');
  });
  it('liefert einen Ersatz, wenn nichts übrig bleibt', () => {
    expect(slugify('***')).toBe('abschnitt');
  });
});

describe('assignHeadingIds', () => {
  it('vergibt IDs aus dem gerenderten Text (Code, Fettdruck) und nummeriert Dopplungen', () => {
    const el = root('<h2>Der <code>harness</code>-Befehl</h2><h2>Doppelt</h2><h3>Doppelt</h3><p>x</p>');
    const refs = assignHeadingIds(el);
    expect(refs).toEqual([
      { id: 'der-harness-befehl', text: 'Der harness-Befehl', level: 2 },
      { id: 'doppelt', text: 'Doppelt', level: 2 },
      { id: 'doppelt-2', text: 'Doppelt', level: 3 },
    ]);
    expect(el.querySelector('#doppelt-2')?.tagName).toBe('H3');
  });
});
