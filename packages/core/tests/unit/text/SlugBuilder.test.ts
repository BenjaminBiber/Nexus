import { describe, it, expect } from 'vitest';
import { SlugBuilder } from '../../../src/text/SlugBuilder.js';

describe('SlugBuilder', () => {
  const slug = (text: string) => new SlugBuilder().build(text);

  it('schreibt klein und ersetzt Leerzeichen durch Bindestriche', () => {
    expect(slug('Login Formular bauen')).toBe('login-formular-bauen');
  });

  it('schreibt Umlaute und ß um', () => {
    expect(slug('Größe Änderung für Übung')).toBe('groesse-aenderung-fuer-uebung');
  });

  it('entfernt Sonderzeichen und fasst Bindestriche zusammen', () => {
    expect(slug('  Token: prüfen!!! (v2) ')).toBe('token-pruefen-v2');
  });

  it('kürzt auf 40 Zeichen am Wortende', () => {
    const result = slug('eine sehr lange Überschrift die weit über vierzig Zeichen hinausgeht');
    expect(result.length).toBeLessThanOrEqual(40);
    expect(result.endsWith('-')).toBe(false);
    expect(result).toBe('eine-sehr-lange-ueberschrift-die-weit');
  });

  it('liefert "ticket" bei leerem Ergebnis', () => {
    expect(slug('???')).toBe('ticket');
  });
});
