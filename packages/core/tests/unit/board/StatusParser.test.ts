import { describe, expect, it } from 'vitest';
import { StatusParser } from '../../../src/board/StatusParser.js';

describe('StatusParser', () => {
  const parser = new StatusParser();

  it.each([
    ['backlog', 'backlog'],
    ['In Planung', 'in_planning'],
    ['verfeinern', 'in_planning'],
    ['geplant', 'planned'],
    ['in-arbeit', 'in_progress'],
    ['WIP', 'in_progress'],
    ['blockiert', 'blocked'],
    ['erledigt', 'done'],
    ['fertig', 'done'],
    ['nicht mehr nötig', 'not_needed'],
    ['hinfaellig', 'not_needed'],
    ['in_progress', 'in_progress'],
  ])('übersetzt „%s“ zu %s', (input, expected) => {
    expect(parser.parse(input)).toBe(expected);
  });

  it('lehnt „wartet“ mit Hinweis ab', () => {
    expect(() => parser.parse('wartet')).toThrow(/abgeleitet/);
  });

  it('lehnt Unbekanntes mit Code status-invalid ab', () => {
    expect(() => parser.parse('irgendwas')).toThrow(expect.objectContaining({ code: 'status-invalid' }));
  });
});
