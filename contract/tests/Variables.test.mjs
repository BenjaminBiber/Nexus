import { describe, expect, it } from 'vitest';
import { Variables } from '../lib/Variables.mjs';

describe('Variables', () => {
  const variables = () => new Variables({ HOME: '/tmp/home', PORT: '4711', NOW: '2026-10-28T08:00:00.000Z' });

  it('setzt einen Platzhalter in einen Text ein', () => {
    expect(variables().apply('Datei unter {{HOME}}/dispatch.yaml')).toBe('Datei unter /tmp/home/dispatch.yaml');
  });

  it('behält den Typ, wenn ein Wert nur aus einem Platzhalter besteht', () => {
    const vars = variables();
    vars.set('wip', 2);
    expect(vars.apply('{{wip}}')).toBe(2);
  });

  it('arbeitet verschachtelt in Listen und Objekten', () => {
    expect(variables().apply({ pfade: ['{{HOME}}'], tief: { now: '{{NOW}}' } })).toEqual({
      pfade: ['/tmp/home'],
      tief: { now: '2026-10-28T08:00:00.000Z' },
    });
  });

  it('meldet einen unbekannten Namen', () => {
    expect(() => variables().apply('{{QUATSCH}}')).toThrow(/Unbekannte Variable \{\{QUATSCH\}\}/);
  });

  describe('geschützte Platzhalter', () => {
    it('macht aus {{!agent}} wieder {{agent}}, ohne einzusetzen', () => {
      expect(variables().apply('{{!agent}} setzt {{!ticketId}} um.')).toBe('{{agent}} setzt {{ticketId}} um.');
    });

    it('ein geschützter Name muss nicht bekannt sein', () => {
      expect(variables().apply('Nur {{!quatsch}}.')).toBe('Nur {{quatsch}}.');
    });

    it('setzt im selben Text gewöhnliche Platzhalter ein und schützt die anderen', () => {
      expect(variables().apply('{{!agent}} im Home {{HOME}}')).toBe('{{agent}} im Home /tmp/home');
    });

    it('schützt auch einen Text, der nur aus einem geschützten Platzhalter besteht', () => {
      expect(variables().apply('{{!agent}}')).toBe('{{agent}}');
    });
  });
});
