import { describe, expect, it } from 'vitest';
import { StepFilter } from '../lib/StepFilter.mjs';

// Ein Szenario mit zwei Aufbauschritten (record: false) und drei aufgezeichneten Schritten
const SZENARIO = {
  name: 'cli/board/tickets-write',
  steps: [
    { name: 'Projekt', record: false },
    { name: 'Board', record: false },
    { name: 'create einfach' },
    { name: 'set Titel und Priorität' },
    { name: 'start eines laufenden Tickets (Fehler)' },
  ],
};

describe('StepFilter', () => {
  // Die Namen der Schritte, die mit dieser --step-Angabe laufen
  const namen = (step) => new StepFilter(step).scenario(SZENARIO).steps.map((s) => s.name);

  it('lässt das Szenario ohne --step unverändert', () => {
    const filter = new StepFilter(null);
    expect(filter.active).toBe(false);
    expect(filter.scenario(SZENARIO)).toBe(SZENARIO);
  });

  it('behält die gewählten Schritte und alle Aufbauschritte', () => {
    expect(namen('create, set ')).toEqual(['Projekt', 'Board', 'create einfach', 'set Titel und Priorität']);
  });

  it('lässt ohne Treffer nur die Aufbauschritte übrig', () => {
    expect(namen('merge')).toEqual(['Projekt', 'Board']);
  });

  it('vergleicht nur mit den Einträgen der gewählten Schritte', () => {
    const entries = [{ name: 'create einfach' }, { name: 'start eines laufenden Tickets (Fehler)' }];
    expect(new StepFilter('create').entries(entries)).toEqual([{ name: 'create einfach' }]);
    expect(new StepFilter(null).entries(entries)).toBe(entries);
  });
});
