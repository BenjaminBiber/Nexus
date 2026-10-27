import { describe, expect, it } from 'vitest';
import { SearchSnippet } from '../../../src/text/SearchSnippet.js';

// Markierte Teile eines Ausschnitts
function marked(snippet: { text: string; matches: Array<[number, number]> }): string[] {
  return snippet.matches.map(([a, b]) => snippet.text.slice(a, b));
}

describe('SearchSnippet', () => {
  const s = new SearchSnippet(60);

  it('markiert Treffer unabhängig von Groß/Klein und mit Umlauten', () => {
    const r = s.build('Die Übersicht zeigt übersichtlich alles', ['übersicht']);
    expect(r.text).toBe('Die Übersicht zeigt übersichtlich alles');
    expect(marked(r)).toEqual(['Übersicht', 'übersicht']);
  });

  it('schneidet um die erste Fundstelle mit Auslassungspunkten', () => {
    const text = `${'a '.repeat(100)}Treffer hier ${'b '.repeat(100)}`;
    const r = s.build(text, ['treffer']);
    expect(r.text.startsWith('…')).toBe(true);
    expect(r.text.endsWith('…')).toBe(true);
    expect(r.text.length).toBeLessThanOrEqual(62);
    expect(marked(r)).toEqual(['Treffer']);
  });

  it('markiert mehrere Begriffe und fasst Überschneidungen zusammen', () => {
    const r = s.build('Board-Suche im Board', ['board', 'oard-s']);
    expect(marked(r)).toEqual(['Board-S', 'Board']);
  });

  it('setzt Auslassungspunkte, wenn der Ausschnitt aus der Mitte eines längeren Feldes stammt', () => {
    const r = s.build('mitten Treffer drin', ['treffer'], { cutBefore: true, cutAfter: true });
    expect(r.text).toBe('…mitten Treffer drin…');
    expect(marked(r)).toEqual(['Treffer']);
  });

  it('fasst Leerraum zusammen und liefert ohne Treffer den Anfang', () => {
    const r = s.build('Zeile 1\n\n   Zeile 2', ['fehlt']);
    expect(r).toEqual({ text: 'Zeile 1 Zeile 2', matches: [] });
  });
});
