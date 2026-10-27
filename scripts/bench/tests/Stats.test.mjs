import { describe, expect, it } from 'vitest';
import { mean, median, p95, percentile, round1 } from '../lib/Stats.mjs';

describe('Stats', () => {
  it('nimmt bei ungerader Anzahl den mittleren Wert als Median', () => {
    expect(median([5, 1, 3])).toBe(3);
  });

  it('nimmt bei gerader Anzahl das Mittel der beiden mittleren Werte als Median', () => {
    expect(median([4, 1, 3, 2])).toBe(2.5);
  });

  it('nimmt bei einem einzigen Wert diesen als Median und p95', () => {
    expect(median([7])).toBe(7);
    expect(p95([7])).toBe(7);
  });

  it('trifft p95 genau, wenn die Position auf einen Messwert fällt', () => {
    // 21 Werte: Position 0,95 * 20 = 19 – also genau der 20. Wert
    expect(p95(Array.from({ length: 21 }, (_, i) => i + 1))).toBe(20);
  });

  it('interpoliert p95 zwischen den Nachbarwerten', () => {
    // 11 Werte: Position 0,95 * 10 = 9,5 – also die Mitte zwischen dem 10. (10) und dem 11. Wert (11)
    expect(p95([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11])).toBeCloseTo(10.5, 10);
  });

  it('sortiert die Reihe vor dem Perzentil', () => {
    expect(percentile([30, 10, 20], 50)).toBe(20);
  });

  it('rechnet den Mittelwert', () => {
    expect(mean([1, 2, 3, 4])).toBe(2.5);
  });

  it('lässt die Eingabe unverändert', () => {
    const werte = [3, 1, 2];
    median(werte);
    expect(werte).toEqual([3, 1, 2]);
  });

  it('wirft bei leerer Liste', () => {
    expect(() => median([])).toThrow(/mindestens einen Messwert/);
    expect(() => p95([])).toThrow(/mindestens einen Messwert/);
    expect(() => mean([])).toThrow(/mindestens einen Messwert/);
  });

  it('wirft bei Werten, die keine endlichen Zahlen sind', () => {
    expect(() => median([1, Number.NaN])).toThrow(/endliche Zahlen/);
    expect(() => mean([1, '2'])).toThrow(/endliche Zahlen/);
  });

  it('wirft bei einem Perzentil außerhalb von 0 bis 100', () => {
    expect(() => percentile([1, 2], 101)).toThrow(/zwischen 0 und 100/);
  });

  it('rundet auf eine Nachkommastelle', () => {
    expect(round1(12.345)).toBe(12.3);
    expect(round1(12.35)).toBe(12.4);
    expect(round1(12)).toBe(12);
  });

  it('wirft beim Runden einer Nicht-Zahl', () => {
    expect(() => round1(Number.POSITIVE_INFINITY)).toThrow(/endliche Zahl/);
  });
});
