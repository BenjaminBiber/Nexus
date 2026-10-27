import { describe, expect, it } from 'vitest';
import { BoardSearchMatch } from '../../../src/board/BoardSearchMatch.js';

describe('BoardSearchMatch', () => {
  it('ignoriert Groß/Klein auch bei Umlauten, achtet aber auf Akzente', () => {
    expect(new BoardSearchMatch(['ärger']).matches('Großer Ärger im Login')).toBe(true);
    expect(new BoardSearchMatch(['arger']).matches('Großer Ärger im Login')).toBe(false);
    expect(new BoardSearchMatch(['ÄRGER']).matches('ärgerlich')).toBe(true);
  });

  it('verlangt alle Begriffe im selben Text und trifft null nie', () => {
    expect(new BoardSearchMatch(['login', 'passwort']).matches('Login bauen')).toBe(false);
    expect(new BoardSearchMatch(['login', 'passwort']).matches('Login mit Passwort')).toBe(true);
    expect(new BoardSearchMatch(['login']).matches(null)).toBe(false);
  });

  it('nimmt Sonderzeichen wörtlich', () => {
    const text = 'Rabatt 100% auf _x_ [a]';
    expect(new BoardSearchMatch(['100%']).matches(text)).toBe(true);
    expect(new BoardSearchMatch(['_x_']).matches(text)).toBe(true);
    expect(new BoardSearchMatch(['[a]']).matches(text)).toBe(true);
    expect(new BoardSearchMatch(['ra%tt']).matches(text)).toBe(false);
  });

  it('liefert kurze Felder ganz', () => {
    expect(new BoardSearchMatch(['oauth']).excerpt('Plan: OAuth einbauen'))
      .toEqual({ text: 'Plan: OAuth einbauen', textStart: 0, textLength: 20 });
  });

  it('schneidet lange Felder um die erste Fundstelle des ersten Begriffs aus', () => {
    const value = `${'x'.repeat(9000)} Fundstelle am Ende`;
    const excerpt = new BoardSearchMatch(['fundstelle']).excerpt(value);
    expect(excerpt.textStart).toBe(8851);
    expect(excerpt.textLength).toBe(9019);
    expect(excerpt.text).toBe(value.slice(8851, 9161));
    expect(excerpt.text.length).toBe(168);
    expect(excerpt.text.endsWith('Fundstelle am Ende')).toBe(true);
  });

  it('beginnt erst hinter dem Fenster zu kürzen', () => {
    expect(new BoardSearchMatch(['treffer']).excerpt(`${'y'.repeat(150)}treffer`).textStart).toBe(0);
    expect(new BoardSearchMatch(['treffer']).excerpt(`${'y'.repeat(151)}treffer`).textStart).toBe(1);
  });
});
