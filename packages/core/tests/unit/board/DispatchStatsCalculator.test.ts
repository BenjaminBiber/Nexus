import { describe, expect, it } from 'vitest';
import { DispatchStatsCalculator, type DispatchStatsFacts } from '../../../src/board/DispatchStatsCalculator.js';
import type { DispatchRun } from '../../../src/board/DispatchRunTypes.js';
import { runFixture } from './helpers/DispatchRunContract.js';

const calc = new DispatchStatsCalculator();
// Ortszeit, damit Wochen und Tage unabhängig von der Zeitzone des Rechners stimmen
const at = (y: number, m: number, d: number, h = 10) => new Date(y, m - 1, d, h).toISOString();

const run = (over: Partial<DispatchRun> & Pick<DispatchRun, 'id' | 'subjectId' | 'outcome' | 'startedAt'>): DispatchRun =>
  runFixture({ projectId: 1, boardId: 5, profile: 'klein', ...over });

// Vier Tickets über den Jahreswechsel (Zeitraum 29.12.2025 bis 11.01.2026):
// A: Zeitlimit in klein, eskaliert nach standard und erledigt
// B: im ersten Lauf erledigt
// C: Schrittlimit in klein, Kontext voll in standard (endgültig)
// D: Frage, dann Pause wegen Nutzungslimit; dazu eine Merge-Sitzung und ein verschwundener Merge-Lauf
function facts(): DispatchStatsFacts {
  return {
    from: new Date(2025, 11, 29), to: new Date(2026, 0, 12), kind: 'work', runsSince: at(2025, 12, 1), configChangedAt: at(2026, 1, 2),
    runs: [
      run({ id: 'a1', subjectId: 'A', outcome: 'abort', abortCategory: 'timeout', startedAt: at(2025, 12, 30) }),
      run({ id: 'a2', subjectId: 'A', outcome: 'done', attempt: 2, profile: 'standard', escalatedFrom: 'klein', startedAt: at(2026, 1, 1) }),
      run({ id: 'b1', subjectId: 'B', outcome: 'review', startedAt: at(2026, 1, 5) }),
      run({ id: 'c1', subjectId: 'C', outcome: 'abort', abortCategory: 'max-turns', startedAt: at(2026, 1, 5) }),
      run({ id: 'c2', subjectId: 'C', outcome: 'abort-final', abortCategory: 'context', attempt: 2, profile: 'standard', escalatedFrom: 'klein', startedAt: at(2026, 1, 6) }),
      run({ id: 'd1', subjectId: 'D', outcome: 'question', startedAt: at(2026, 1, 6) }),
      run({ id: 'd2', subjectId: 'D', outcome: 'pause-limit', startedAt: at(2026, 1, 7) }),
      run({ id: 'm1', subjectId: 'A', kind: 'merge', outcome: 'merge-session', startedAt: at(2026, 1, 1, 11) }),
      run({ id: 'm2', subjectId: 'B', kind: 'merge', outcome: 'abort', abortCategory: 'vanished', startedAt: at(2026, 1, 5, 11) }),
    ],
  };
}

describe('DispatchStatsCalculator', () => {
  it('rechnet die Kennzahlen der Arbeitsläufe wie von Hand', () => {
    const stats = calc.calculate(facts());
    expect(stats.kind).toBe('work');
    expect(stats.configChangedAt).toBe(at(2026, 1, 2));
    expect(stats.kpis).toEqual({
      runs: 7, subjects: 4, done: 2, aborted: 3, solved: 2,
      successRate: 2 / 3, // A und B gelöst, C endgültig; D nicht abgeschlossen
      firstTryRate: 1 / 3, // A nach Eskalation, B im ersten Lauf, C endgültig; D nicht abgeschlossen
      escalationRate: 2 / 4, // A und C
      finalFailures: 1,
    });
    expect(stats.ladder).toEqual({ finished: 3, firstTry: 1 / 3, afterEscalation: 1 / 3, final: 1 / 3 });
  });

  it('gruppiert die Ausgänge je Profil', () => {
    expect(calc.calculate(facts()).byProfile).toEqual([
      { profile: 'klein', runs: 5, done: 1, escalated: 2, final: 0, other: 2 },
      { profile: 'standard', runs: 2, done: 1, escalated: 0, final: 1, other: 0 },
    ]);
  });

  it('zählt Abbruchgründe und den Anteil, den die Einstellung beeinflusst', () => {
    const stats = calc.calculate(facts());
    expect(stats.abortReasons).toEqual([
      { category: 'context', label: 'Kontext voll', count: 1 },
      { category: 'max-turns', label: 'Schrittlimit', count: 1 },
      { category: 'timeout', label: 'Zeitlimit', count: 1 },
    ]);
    expect(stats.configurableAbortShare).toBe(1);
  });

  it('zählt Nutzungslimit-Pausen und hängende Läufe über alle Arten', () => {
    const stats = calc.calculate(facts());
    expect(stats.usagePauses).toBe(1);
    expect(stats.staleRuns).toBe(1);
  });

  it('wertet eine andere Lauf-Art aus (Merge-Sitzung zählt als Erfolg)', () => {
    const stats = calc.calculate({ ...facts(), kind: 'merge' });
    // B nur abgebrochen, nicht endgültig: zählt nicht zur Erfolgsquote
    expect(stats.kpis).toMatchObject({ runs: 2, subjects: 2, done: 1, aborted: 1, solved: 1, successRate: 1 });
    expect(stats.abortReasons).toEqual([{ category: 'vanished', label: 'Worker verschwunden', count: 1 }]);
    expect(stats.configurableAbortShare).toBe(0);
  });

  it('zählt bis 28 Tage je Tag (Ortszeit), mit Fragen und Blockiert als Rest', () => {
    const { series } = calc.calculate(facts());
    expect(series.unit).toBe('day');
    expect(series.points).toHaveLength(14);
    expect(series.points[0].start).toBe('2025-12-29');
    expect(series.points.filter((p) => p.done || p.aborted || p.other)).toEqual([
      { start: '2025-12-30', done: 0, aborted: 1, other: 0 },
      { start: '2026-01-01', done: 1, aborted: 0, other: 0 },
      { start: '2026-01-05', done: 1, aborted: 1, other: 0 },
      { start: '2026-01-06', done: 0, aborted: 1, other: 1 },
    ]);
    // Genau 28 Tage noch je Tag, 29 je Woche
    expect(calc.calculate({ ...facts(), to: new Date(2026, 0, 26) }).series.unit).toBe('day');
    expect(calc.calculate({ ...facts(), to: new Date(2026, 0, 27) }).series.unit).toBe('week');
  });

  it('zählt über 28 Tage je ISO-Woche (Montag, Ortszeit) über den Jahreswechsel', () => {
    const { series } = calc.calculate({ ...facts(), from: new Date(2025, 11, 1) });
    expect(series.unit).toBe('week');
    expect(series.points.map((p) => p.start)).toEqual(['2025-12-01', '2025-12-08', '2025-12-15', '2025-12-22', '2025-12-29', '2026-01-05']);
    expect(series.points.slice(-2)).toEqual([
      { start: '2025-12-29', done: 1, aborted: 1, other: 0 },
      { start: '2026-01-05', done: 1, aborted: 2, other: 1 },
    ]);
  });

  it('liefert für einen leeren Zeitraum null statt einer Division durch 0', () => {
    const stats = calc.calculate({ ...facts(), runs: [], runsSince: null, configChangedAt: null });
    expect(stats.kpis).toEqual({
      runs: 0, subjects: 0, done: 0, aborted: 0, solved: 0, successRate: null, firstTryRate: null, escalationRate: null, finalFailures: 0,
    });
    expect(stats.ladder).toEqual({ finished: 0, firstTry: null, afterEscalation: null, final: null });
    expect(stats.configurableAbortShare).toBeNull();
    expect(stats.series.points.map((p) => p.done)).toEqual(Array(14).fill(0));
    expect(stats.byProfile).toEqual([]);
  });

  it('zählt einen Wiederholungslauf ohne Eskalation nicht als Erstversuch', () => {
    const stats = calc.calculate({ ...facts(), runs: [
      run({ id: 'x2', subjectId: 'X', outcome: 'done', attempt: 2, startedAt: at(2026, 1, 6) }),
    ] });
    expect(stats.kpis).toMatchObject({ firstTryRate: 0, escalationRate: 1 });
  });
});
