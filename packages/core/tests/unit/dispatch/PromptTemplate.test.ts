import { describe, it, expect } from 'vitest';
import { PromptTemplate } from '../../../src/dispatch/PromptTemplate.js';

describe('PromptTemplate.render', () => {
  const t = new PromptTemplate();

  it('ersetzt einfache Platzhalter, auch mehrfach vorkommende', () => {
    const out = t.render('Ticket {{ticketId}} an {{agent}} ({{ticketId}})', { ticketId: 'AIH-0086', agent: 'junior' });
    expect(out).toBe('Ticket AIH-0086 an junior (AIH-0086)');
  });

  it('setzt mehrzeilige Werte und Array-Werte unveraendert ein', () => {
    const out = t.render('vorher\n{{answers}}\nnachher', { answers: 'A\nB' });
    expect(out).toBe('vorher\nA\nB\nnachher');
    const arr = t.render('vorher\n{{answers}}\nnachher', { answers: ['A', 'B'] });
    expect(arr).toBe('vorher\nA\nB\nnachher');
  });

  it('meldet unbekannte Platzhalter mit Namen in einer Meldung', () => {
    expect(() => t.render('{{foo}} und {{bar}} und {{agent}}', { agent: 'x' }))
      .toThrow(/\{\{foo\}\}, \{\{bar\}\}/);
  });

  it('laesst eine Zeile mit nur einem leeren Block-Platzhalter ganz wegfallen', () => {
    expect(t.render('eins\n{{answers}}\nzwei', { answers: '' })).toBe('eins\nzwei');
    expect(t.render('eins\n{{answers}}\nzwei', { answers: [] })).toBe('eins\nzwei');
  });

  it('entfernt die Zeile nicht, wenn sie neben dem leeren Platzhalter Text enthaelt', () => {
    expect(t.render('eins\nText {{answers}}\nzwei', { answers: '' })).toBe('eins\nText \nzwei');
  });

  it('behaelt die Einrueckung einer nicht-leeren Block-Zeile', () => {
    expect(t.render('eins\n  {{answers}}\nzwei', { answers: 'A' })).toBe('eins\n  A\nzwei');
  });

  it('laesst Text ohne Platzhalter und fehlerhafte Klammern unveraendert', () => {
    expect(t.render('nur {text} und {{offen', {})).toBe('nur {text} und {{offen');
  });

  it('kommt mit \\r\\n-Zeilenenden zurecht', () => {
    expect(t.render('eins\r\n{{answers}}\r\nzwei', { answers: '' })).toBe('eins\r\nzwei');
  });
});

describe('PromptTemplate.names', () => {
  const t = new PromptTemplate();

  it('liefert die Namen in Reihenfolge ohne Dubletten', () => {
    expect(t.names('{{b}} {{a}} {{b}}')).toEqual(['b', 'a']);
  });

  it('liefert fuer Text ohne Platzhalter eine leere Liste', () => {
    expect(t.names('kein Platzhalter {hier}')).toEqual([]);
  });
});
