import { describe, expect, it } from 'vitest';
import type { DispatchConfigVersion, DispatchRun } from '../../../../src/board/DispatchRunTypes.js';
import type { RawDispatchConfig } from '../../../../src/dispatch/DispatchConfig.js';
import { DispatchConfigFingerprint } from '../../../../src/dispatch/DispatchConfigFingerprint.js';
import { DispatchConfigNormalizer } from '../../../../src/dispatch/DispatchConfigNormalizer.js';
import { OptimizationFactsCollector, sortedJson } from '../../../../src/dispatch/optimization/OptimizationFactsCollector.js';
import { runFixture } from '../../board/helpers/DispatchRunContract.js';

const normalizer = new DispatchConfigNormalizer();
const RAW: RawDispatchConfig = {
  ...normalizer.example(),
  profiles: {
    klein: { model: 'sonnet', maxTurns: 20, timeoutMinutes: 30 },
    standard: { model: 'opus', maxTurns: 30, timeoutMinutes: 60 },
    gross: { model: 'opus', maxTurns: 80, timeoutMinutes: 120 },
  },
  defaultProfile: 'standard', escalate: true, escalation: ['klein', 'standard', 'gross'],
  rules: [{ when: { labels: ['doku'] }, profile: 'klein' }],
  research: { auto: false, profile: null }, planning: { profile: null },
};
const CONFIG = normalizer.normalize(RAW);
const LIMITS = { standard: [30, 60], klein: [20, 30], gross: [80, 120] } as const;

// Lauf mit den aktuellen Limits seines Profils
function run(id: string, over: Partial<DispatchRun> = {}): DispatchRun {
  const profile = (over.profile ?? 'standard') as keyof typeof LIMITS;
  return runFixture({ id, subjectId: id, projectId: 1, profile, maxTurns: LIMITS[profile][0], timeoutMinutes: LIMITS[profile][1], ...over });
}
const collect = (runs: DispatchRun[], versions: DispatchConfigVersion[] = []) =>
  new OptimizationFactsCollector().collect({ runs, config: CONFIG, versions, since: '2026-04-01T00:00:00.000Z' });

describe('OptimizationFactsCollector', () => {
  it('zählt je Profil nur Läufe mit den aktuellen Limits, den letzten Lauf aber unabhängig davon', () => {
    const f = collect([
      run('a', { startedAt: '2026-10-01T00:00:00.000Z' }),
      run('b', { maxTurns: 20, startedAt: '2026-10-02T00:00:00.000Z' }),
    ]).profiles.find((p) => p.profile === 'standard')!;
    expect(f).toMatchObject({ runs: 1, starts: 1, maxTurns: 30, timeoutMinutes: 60, lastRunAt: '2026-10-02T00:00:00.000Z' });
  });

  it('verfolgt Eskalationen: überholte Starts, Ankünfte per Eskalation, häufigste Herkunft und Wege', () => {
    const facts = collect([
      run('T1'),
      run('T1-2', { subjectId: 'T1', profile: 'gross', escalatedFrom: 'standard', attempt: 2 }),
      run('T2'),
    ]);
    expect(facts.profiles.find((p) => p.profile === 'standard')).toMatchObject({ starts: 2, escalatedStarts: 1 });
    expect(facts.profiles.find((p) => p.profile === 'gross')).toMatchObject({ runs: 1, starts: 0, viaEscalation: 1, topOrigin: 'standard' });
    expect(facts.paths).toEqual([{ from: 'standard', to: 'gross', count: 1 }]);
  });

  it('zählt Abbruchgründe, Erfolg im ersten Versuch, P90 der Schritte und Herkunft der Starts', () => {
    const runs = Array.from({ length: 10 }, (_, i) => run(`r${i}`, {
      numTurns: i + 1,
      outcome: i < 7 ? 'done' : 'abort',
      abortCategory: i >= 7 ? 'max-turns' : null,
      profileReason: i < 2 ? 'rule' : 'default',
      ruleIndex: i < 2 ? 1 : null,
    }));
    expect(collect(runs).profiles.find((p) => p.profile === 'standard')).toMatchObject({
      runs: 10, aborts: { 'max-turns': 3 }, firstTrySuccess: 7, turnsP90: 9,
      startSources: [{ reason: 'default', ruleIndex: null, count: 8 }, { reason: 'rule', ruleIndex: 1, count: 2 }],
    });
  });

  it('wertet Regeln nur für Läufe aus, deren Konfig-Fassung dieselbe Regel an dieser Stelle hatte', () => {
    const fp = new DispatchConfigFingerprint();
    const same = { hash: 'same', dispatcher: 'd', firstSeenAt: '2026-09-01T00:00:00.000Z', content: fp.content(CONFIG) };
    const other = normalizer.normalize({ ...RAW, rules: [{ when: { labels: ['alt'] }, profile: 'klein' }] });
    const changed = { hash: 'old', dispatcher: 'd', firstSeenAt: '2026-08-01T00:00:00.000Z', content: fp.content(other) };
    const viaRule = { profile: 'klein', profileReason: 'rule' as const, ruleIndex: 1 };
    const facts = collect([
      run('A', { ...viaRule, configHash: 'same', outcome: 'abort-final' }),
      run('B', { ...viaRule, configHash: 'same' }),
      run('B-2', { subjectId: 'B', profile: 'standard', escalatedFrom: 'klein', attempt: 2 }),
      run('C', { ...viaRule, configHash: 'same' }),
      run('D', { ...viaRule, configHash: 'old' }),
      run('E', { ...viaRule, configHash: null }),
    ], [same, changed]);
    expect(facts.rules).toEqual([{ index: 1, profile: 'klein', runs: 3, failed: 2 }]);
  });

  it('beschreibt die Basis: nur Arbeitsläufe, Zahl der Projekte und Zeitraum', () => {
    const facts = collect([run('a'), run('b', { projectId: 2 }), run('m', { kind: 'merge', projectId: 3 })]);
    expect(facts.basis).toEqual({ runs: 2, projects: 2, since: '2026-04-01T00:00:00.000Z' });
  });

  it('vergleicht Objekte unabhängig von der Reihenfolge der Schlüssel', () => {
    expect(sortedJson({ b: 1, a: { d: 2, c: 3 } })).toBe(sortedJson({ a: { c: 3, d: 2 }, b: 1 }));
  });
});
