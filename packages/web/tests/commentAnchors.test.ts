// @vitest-environment jsdom
import { describe, it, expect, afterEach } from 'vitest';
import { anchorIn, clearHighlights, highlightQuote, lineRangeOf, readSelection } from '../src/markdown/commentAnchors.js';

const roots: HTMLElement[] = [];

// Räumt die angehängten Wurzeln nach jedem Test weg
afterEach(() => {
  roots.splice(0).forEach((el) => el.remove());
});

// Baut ein Wurzelelement aus HTML und hängt es ins Dokument (für Selection)
function root(html: string): HTMLElement {
  const el = document.createElement('div');
  el.innerHTML = html;
  document.body.append(el);
  roots.push(el);
  return el;
}

// Markiert den Text im ersten Absatz von Zeichen 4 bis 10
function selectInParagraph(el: HTMLElement): void {
  const text = el.querySelector('p')!.firstChild!;
  const range = document.createRange();
  range.setStart(text, 4);
  range.setEnd(text, 10);
  const selection = window.getSelection()!;
  selection.removeAllRanges();
  selection.addRange(range);
}

describe('highlightQuote', () => {
  it('hebt ein Zitat in einem Textknoten hervor', () => {
    const el = root('<p>Das Ticket wird angelegt.</p>');
    expect(highlightQuote(el, 'Ticket wird', 'K-1')).toBe(true);
    const mark = el.querySelector('mark.doc-comment') as HTMLElement;
    expect(mark.textContent).toBe('Ticket wird');
    expect(mark.dataset.commentId).toBe('K-1');
    expect(el.textContent).toBe('Das Ticket wird angelegt.');
  });

  it('hebt ein Zitat über mehrere Elemente hinweg hervor', () => {
    const el = root('<p>Das <strong>wichtige</strong> Ticket</p>');
    expect(highlightQuote(el, 'wichtige Ticket', 'K-2')).toBe(true);
    expect(Array.from(el.querySelectorAll('mark.doc-comment')).map((m) => m.textContent).join('')).toBe('wichtige Ticket');
  });

  it('meldet false, wenn das Zitat fehlt, und räumt Hervorhebungen auf', () => {
    const el = root('<p>Anderer Text</p>');
    expect(highlightQuote(el, 'Ticket', 'K-3')).toBe(false);
    highlightQuote(el, 'Text', 'K-4');
    clearHighlights(el);
    expect(el.querySelector('mark')).toBeNull();
    expect(el.innerHTML).toBe('<p>Anderer Text</p>');
  });

  it('findet ein Zitat über zwei Absätze (Leerraum normalisiert)', () => {
    const el = root('<p>Erster Satz.</p><p>Zweiter Satz.</p>');
    expect(highlightQuote(el, 'Satz. Zweiter', 'K-5')).toBe(true);
    const marks = Array.from(el.querySelectorAll('mark.doc-comment'));
    expect(marks.map((m) => m.textContent).join('')).toBe('Satz.Zweiter');
    expect(el.textContent).toBe('Erster Satz.Zweiter Satz.');
  });

  it('findet ein Zitat über Zeilenumbruch im Quelltext des Textknotens', () => {
    const el = root('<p>Das Ticket\nwird angelegt.</p>');
    expect(highlightQuote(el, 'Ticket wird', 'K-6')).toBe(true);
    expect(el.querySelector('mark.doc-comment')!.textContent).toBe('Ticket\nwird');
  });

  it('hebt bei doppeltem Vorkommen die erste Stelle hervor', () => {
    const el = root('<p>Ticket eins und Ticket zwei</p>');
    expect(highlightQuote(el, 'Ticket', 'K-7')).toBe(true);
    const marks = el.querySelectorAll('mark.doc-comment');
    expect(marks).toHaveLength(1);
    expect(marks[0].nextSibling!.textContent).toBe(' eins und Ticket zwei');
  });

  it('packt Leerraum zwischen Listenpunkten nicht in eine Hervorhebung', () => {
    const el = root('<ul>\n<li>Eins</li>\n<li>Zwei</li>\n</ul>');
    expect(highlightQuote(el, 'Eins Zwei', 'K-9')).toBe(true);
    expect(Array.from(el.querySelector('ul')!.children).every((c) => c.tagName === 'LI')).toBe(true);
    expect(Array.from(el.querySelectorAll('mark.doc-comment')).map((m) => m.textContent).join('')).toBe('EinsZwei');
  });

  it('lässt Such-Marks ohne Klasse bei clearHighlights unberührt', () => {
    const el = root('<p>Ein <mark>Treffer</mark> und Text</p>');
    highlightQuote(el, 'und Text', 'K-8');
    clearHighlights(el);
    expect(el.querySelectorAll('mark')).toHaveLength(1);
    expect(el.querySelector('mark')!.textContent).toBe('Treffer');
    expect(el.innerHTML).toBe('<p>Ein <mark>Treffer</mark> und Text</p>');
  });
});

describe('readSelection', () => {
  it('liefert Zitat und vorangehende Überschrift', () => {
    const el = root('<h2>Ziel</h2><p>Das Ticket wird angelegt.</p>');
    const text = el.querySelector('p')!.firstChild!;
    const range = document.createRange();
    range.setStart(text, 4);
    range.setEnd(text, 10);
    const selection = window.getSelection()!;
    selection.removeAllRanges();
    selection.addRange(range);
    expect(readSelection(el)).toEqual({ quote: 'Ticket', heading: 'Ziel' });
  });

  it('liefert die nächste Unterüberschrift und trimmt ihren Text', () => {
    const el = root('<h2>Ziel</h2><h3> Detail </h3><p>Das Ticket wird angelegt.</p>');
    selectInParagraph(el);
    expect(readSelection(el)).toEqual({ quote: 'Ticket', heading: 'Detail' });
  });

  it('liefert keine Überschrift, wenn nur eine h1 davor steht', () => {
    const el = root('<h1>Titel</h1><p>Das Ticket wird angelegt.</p>');
    selectInParagraph(el);
    expect(readSelection(el)).toEqual({ quote: 'Ticket', heading: null });
  });

  it('liefert null ohne Auswahl', () => {
    const el = root('<p>x</p>');
    window.getSelection()!.removeAllRanges();
    expect(readSelection(el)).toBeNull();
  });
});

describe('lineRangeOf', () => {
  it('findet die Zeilen eines Zitats im Markdown', () => {
    const source = '# Titel\n\nDas Ticket wird\nangelegt.\n';
    expect(lineRangeOf(source, 'Ticket wird')).toEqual([3, 3]);
  });
  it('liefert null, wenn das Zitat im Quelltext nicht vorkommt', () => {
    expect(lineRangeOf('# Titel', 'Unbekannt')).toBeNull();
  });
  it('findet gerenderten Text trotz Inline-Code, Fett und Links', () => {
    const source = '# Titel\n\n**Architecture:** Reine Core-Bausteine (`OptimizationFactsCollector` verdichtet Läufe, `DISPATCH_SECTIONS`).\nSiehe [die Spec](docs/spec.md) dazu.\n';
    expect(lineRangeOf(source, 'Reine Core-Bausteine ( OptimizationFactsCollector')).toEqual([3, 3]);
    expect(lineRangeOf(source, 'Architecture: Reine')).toEqual([3, 3]);
    expect(lineRangeOf(source, 'DISPATCH_SECTIONS). Siehe die Spec dazu')).toEqual([3, 4]);
  });
  it('reicht über Listenpunkte und Zeilenumbrüche bis zur letzten Zeile des Zitats', () => {
    const source = 'Vorher\n\n- Eins\n- Zwei\n  geht weiter\n\n## Danach\n';
    expect(lineRangeOf(source, 'Eins Zwei geht weiter')).toEqual([3, 5]);
  });
});

describe('anchorIn', () => {
  // Layout mit festem Rechteck, da jsdom keine Geometrie berechnet
  function layout(): HTMLElement {
    const el = document.createElement('div');
    el.getBoundingClientRect = () => ({ top: 100, left: 50, width: 800, height: 600 }) as DOMRect;
    return el;
  }

  it('setzt das Popover unter die Auswahl, relativ zum Layout', () => {
    const rect = { bottom: 220, left: 150 } as DOMRect;
    expect(anchorIn(layout(), rect)).toEqual({ top: 126, left: 100 });
  });

  it('hält das Popover im Layout und fällt ohne Rechteck auf 0 zurück', () => {
    expect(anchorIn(layout(), { bottom: 220, left: 5000 } as DOMRect).left).toBe(480);
    expect(anchorIn(layout(), null)).toEqual({ top: 0, left: 0 });
  });
});
