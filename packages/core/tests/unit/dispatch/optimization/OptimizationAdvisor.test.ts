import { describe, expect, it } from 'vitest';
import type { RawDispatchConfig } from '../../../../src/dispatch/DispatchConfig.js';
import { DispatchConfigNormalizer } from '../../../../src/dispatch/DispatchConfigNormalizer.js';
import { OptimizationAdvisor } from '../../../../src/dispatch/optimization/OptimizationAdvisor.js';
import type { OptimizationFacts, ProfileFacts, RuleFacts } from '../../../../src/dispatch/optimization/SuggestionTypes.js';

const normalizer = new DispatchConfigNormalizer();
const RAW: RawDispatchConfig = {
  ...normalizer.example(),
  profiles: {
    klein: { model: 'sonnet', maxTurns: 20, timeoutMinutes: 30 },
    standard: { model: 'opus', maxTurns: 30, timeoutMinutes: 60 },
    gross: { model: 'opus', maxTurns: 80, timeoutMinutes: 120 },
    alt: { model: 'sonnet', maxTurns: 10, timeoutMinutes: 10 },
  },
  defaultProfile: 'standard', escalate: true, escalation: ['klein', 'standard', 'gross'],
  rules: [{ when: { labels: ['doku'] }, profile: 'klein' }],
  research: { auto: false, profile: null }, planning: { profile: null },
};
const CONFIG = normalizer.normalize(RAW);
const NOW = new Date('2026-10-22T12:00:00.000Z');
const LIMITS: Record<string, [number, number]> = { klein: [20, 30], standard: [30, 60], gross: [80, 120], alt: [10, 10] };

// Unauffälliges Profil mit 10 Läufen; einzelne Werte überschreibbar
function pf(profile: string, over: Partial<ProfileFacts> = {}): ProfileFacts {
  const [maxTurns, timeoutMinutes] = LIMITS[profile]!;
  return {
    profile, maxTurns, timeoutMinutes, runs: 10, starts: 10, escalatedStarts: 0, viaEscalation: 0, topOrigin: null, aborts: {},
    firstTrySuccess: 5, turnsP90: Math.round(maxTurns * 0.8), startSources: [{ reason: 'default', ruleIndex: null, count: 10 }],
    lastRunAt: '2026-10-20T00:00:00.000Z', ...over,
  };
}
function facts(profiles: ProfileFacts[], rules: RuleFacts[] = []): OptimizationFacts {
  const all = ['klein', 'standard', 'gross', 'alt'].map((p) => profiles.find((x) => x.profile === p) ?? pf(p));
  return { basis: { runs: 40, projects: 2, since: '2026-04-25T00:00:00.000Z' }, profiles: all, rules, paths: [] };
}
const advise = (f: OptimizationFacts) => new OptimizationAdvisor().advise(f, CONFIG, NOW);
const kinds = (f: OptimizationFacts) => advise(f).suggestions.map((s) => s.kind);

describe('OptimizationAdvisor', () => {
  it('schlägt bei ≥ 20 % Abbrüchen wegen Schritten 1,5-fache Schritte vor, auf 5 gerundet', () => {
    const [s] = advise(facts([pf('standard', { aborts: { 'max-turns': 2 } })])).suggestions;
    expect(s).toMatchObject({
      kind: 'turns-too-low', source: 'rule', title: 'Schritte zu knapp · standard', target: { profile: 'standard' },
      patch: [{ path: 'profiles.standard.maxTurns', from: 30, to: 45 }], evidence: { runs: 10, hits: 2, rate: 0.2 },
    });
    expect(s!.reason).toContain('2 von 10 Läufen (20 %)');
    expect(s!.id).toMatch(/^[0-9a-f]{16}$/);
    expect(kinds(facts([pf('standard', { aborts: { 'max-turns': 1 } })]))).not.toContain('turns-too-low');
  });

  it('schlägt bei ≥ 20 % Zeitlimit-Abbrüchen ein längeres Zeitlimit vor', () => {
    expect(advise(facts([pf('standard', { aborts: { timeout: 3 } })])).suggestions[0]).toMatchObject({
      kind: 'timeout-too-low', patch: [{ path: 'profiles.standard.timeoutMinutes', from: 60, to: 90 }],
    });
  });

  it('meldet vollen Kontext ab 15 % nur als Hinweis', () => {
    expect(advise(facts([pf('standard', { aborts: { context: 2 } })])).suggestions[0]).toMatchObject({ kind: 'context-full', patch: null });
    expect(kinds(facts([pf('standard', { aborts: { context: 1 } })]))).not.toContain('context-full');
  });

  it('urteilt unter 5 Läufen nicht und meldet genutzte Profile als „zu wenig Daten“', () => {
    const result = advise(facts([pf('standard', { runs: 4, starts: 4, aborts: { 'max-turns': 4 } })]));
    expect(result.suggestions.map((s) => s.kind)).not.toContain('turns-too-low');
    expect(result.insufficient).toEqual([{ profile: 'standard', runs: 4, needed: 5 }]);
  });

  it('lenkt ein meist überholtes Standardprofil auf die nächste Stufe der Leiter', () => {
    expect(advise(facts([pf('standard', { escalatedStarts: 5 })])).suggestions[0]).toMatchObject({
      kind: 'profile-overtaken', patch: [{ path: 'defaultProfile', from: 'standard', to: 'gross' }],
    });
  });

  it('lenkt ein meist überholtes Profil über die Regel um, aus der es meist kommt', () => {
    const klein = pf('klein', { escalatedStarts: 6, startSources: [{ reason: 'rule', ruleIndex: 1, count: 10 }] });
    expect(advise(facts([klein])).suggestions[0]).toMatchObject({
      kind: 'profile-overtaken', target: { profile: 'klein', ruleIndex: 1 },
      patch: [{ path: 'rules.0.profile', from: 'klein', to: 'standard' }],
    });
  });

  it('gibt für die oberste Stufe der Leiter nur einen Hinweis', () => {
    expect(advise(facts([pf('gross', { escalatedStarts: 8 })])).suggestions[0]).toMatchObject({ kind: 'profile-overtaken', patch: null });
  });

  it('macht ein Eskalationsziel, das öfter per Eskalation als direkt genutzt wird, zum Start und fasst gleiche Änderungen zusammen', () => {
    const gross = pf('gross', { runs: 8, starts: 2, viaEscalation: 6, topOrigin: 'standard' });
    const standard = pf('standard', { escalatedStarts: 5 });
    const result = advise(facts([gross, standard]));
    const patched = result.suggestions.filter((s) => s.patch?.[0]?.path === 'defaultProfile');
    expect(patched).toHaveLength(1);
    expect(patched[0]).toMatchObject({ kind: 'escalation-target-dominates', patch: [{ path: 'defaultProfile', from: 'standard', to: 'gross' }] });
  });

  it('hebt das Ziel einer Regel an, deren Tickets ≥ 50 % eskaliert werden oder scheitern', () => {
    expect(advise(facts([], [{ index: 1, profile: 'klein', runs: 6, failed: 3 }])).suggestions[0]).toMatchObject({
      kind: 'rule-fails', title: 'Regel 1 führt in Abbrüche', target: { profile: 'klein', ruleIndex: 1 },
      patch: [{ path: 'rules.0.profile', from: 'klein', to: 'standard' }],
    });
    expect(kinds(facts([], [{ index: 1, profile: 'klein', runs: 4, failed: 4 }]))).not.toContain('rule-fails');
  });

  it('hält ein höheres Profil mit fast immer glattem ersten Versuch und wenig Schritten für eventuell zu groß', () => {
    expect(advise(facts([pf('gross', { firstTrySuccess: 9, turnsP90: 32 })])).suggestions[0]).toMatchObject({ kind: 'profile-oversized', patch: null });
    expect(kinds(facts([pf('gross', { firstTrySuccess: 9, turnsP90: 33 })]))).not.toContain('profile-oversized');
    expect(kinds(facts([pf('klein', { firstTrySuccess: 10, turnsP90: 1 })]))).not.toContain('profile-oversized');
  });

  it('meldet ein Profil außerhalb von Leiter und Regeln ohne Lauf seit 30 Tagen als ungenutzt', () => {
    expect(advise(facts([pf('alt', { runs: 0, starts: 0, lastRunAt: null })])).suggestions[0]).toMatchObject({ kind: 'profile-unused', patch: null });
    expect(kinds(facts([pf('alt', { runs: 0, starts: 0, lastRunAt: '2026-10-17T00:00:00.000Z' })]))).not.toContain('profile-unused');
    expect(advise(facts([pf('alt', { runs: 0, starts: 0, lastRunAt: null })])).insufficient).toEqual([]);
  });

  it('sortiert Übernehmbares vor Hinweisen und ändert die ID, wenn sich das Profil ändert', () => {
    const list = advise(facts([pf('standard', { aborts: { context: 5, 'max-turns': 2 } })])).suggestions;
    expect(list.map((s) => s.kind)).toEqual(['turns-too-low', 'context-full']);
    const before = list[0]!.id;
    const after = advise(facts([pf('standard', { maxTurns: 45, aborts: { 'max-turns': 2 } })])).suggestions[0]!.id;
    expect(after).not.toBe(before);
  });
});
