import { describe, expect, it } from 'vitest';
import { fitTabs } from '../src/components/fitTabs.js';

// Fünf Reiter zu je 100 px, 2 px Abstand dazwischen, ⋯-Knopf 40 px
const WIDTHS = [100, 100, 100, 100, 100];
const base = { widths: WIDTHS, gap: 2, moreWidth: 40 };

describe('fitTabs', () => {
  it('zeigt alle Reiter, wenn sie in die Leiste passen', () => {
    expect(fitTabs({ ...base, available: 508, active: 0 })).toEqual({ visible: [0, 1, 2, 3, 4], hidden: [] });
  });

  it('zeigt alle Reiter, solange keine Breite gemessen ist', () => {
    expect(fitTabs({ ...base, available: 0, active: 4 })).toEqual({ visible: [0, 1, 2, 3, 4], hidden: [] });
  });

  it('legt Reiter, die nicht passen, ins Menü und hält Platz für den ⋯-Knopf frei', () => {
    // 3 Reiter + 3 Abstände + Knopf = 346 px; der vierte passt mit Knopf nicht mehr
    expect(fitTabs({ ...base, available: 400, active: 0 })).toEqual({ visible: [0, 1, 2], hidden: [3, 4] });
  });

  it('hält den aktiven Reiter sichtbar, er tritt an die Stelle des letzten sichtbaren', () => {
    expect(fitTabs({ ...base, available: 400, active: 4 })).toEqual({ visible: [0, 1, 4], hidden: [2, 3] });
  });

  it('zeigt den aktiven Reiter auch, wenn nicht einmal er neben den Knopf passt', () => {
    expect(fitTabs({ ...base, available: 60, active: 3 })).toEqual({ visible: [3], hidden: [0, 1, 2, 4] });
  });

  it('macht Platz für einen breiten aktiven Reiter, indem es weitere Reiter ins Menü legt', () => {
    const widths = [100, 100, 100, 250];
    expect(fitTabs({ widths, gap: 2, moreWidth: 40, available: 400, active: 3 })).toEqual({ visible: [0, 3], hidden: [1, 2] });
  });
});
