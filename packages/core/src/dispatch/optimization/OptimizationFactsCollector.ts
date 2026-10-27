import type { DispatchConfigVersion, DispatchRun } from '../../board/DispatchRunTypes.js';
import type { DispatchConfig } from '../DispatchConfig.js';
import type { EscalationPath, OptimizationFacts, ProfileFacts, RuleFacts, StartSource } from './SuggestionTypes.js';

export interface FactsInput {
  runs: DispatchRun[];
  config: DispatchConfig;
  /** Fassungen der Einstellung (für die Zuordnung der Regeln) */
  versions: DispatchConfigVersion[];
  /** Beginn des Zeitraums (ISO) */
  since: string;
}

// JSON mit sortierten Schlüsseln – vergleicht Regeln unabhängig von der Reihenfolge ihrer Felder
export function sortedJson(value: unknown): string {
  const sort = (v: unknown): unknown => {
    if (Array.isArray(v)) return v.map(sort);
    if (v === null || typeof v !== 'object') return v;
    const out: Record<string, unknown> = {};
    for (const key of Object.keys(v as Record<string, unknown>).sort()) out[key] = sort((v as Record<string, unknown>)[key]);
    return out;
  };
  return JSON.stringify(sort(value));
}

// Erster Lauf eines Tickets auf seinem Basisprofil (keine Eskalation, erster Versuch)
const isStart = (r: DispatchRun) => r.escalatedFrom === null && (r.attempt ?? 1) <= 1;
const SUCCESS = new Set(['done', 'review']);

// Verdichtet die Arbeitsläufe zu Kennzahlen je Profil, je Regel und je Eskalationsweg (Grundlage der Optimierungs-Vorschläge).
export class OptimizationFactsCollector {
  collect(input: FactsInput): OptimizationFacts {
    const work = input.runs.filter((r) => r.kind === 'work');
    const escalated = this.escalatedSubjects(work);
    return {
      basis: { runs: work.length, projects: new Set(work.map((r) => r.projectId)).size, since: input.since },
      profiles: Object.entries(input.config.profiles).map(([name, p]) => this.profile(name, p.maxTurns, p.timeoutMinutes, work, escalated)),
      rules: this.rules(input, work, escalated),
      paths: this.paths(work),
    };
  }

  // Basisprofil → Tickets, die von ihm weg eskaliert wurden
  private escalatedSubjects(work: DispatchRun[]): Map<string, Set<string>> {
    const out = new Map<string, Set<string>>();
    for (const r of work) {
      if (r.escalatedFrom === null) continue;
      const set = out.get(r.escalatedFrom) ?? new Set<string>();
      set.add(r.subjectId);
      out.set(r.escalatedFrom, set);
    }
    return out;
  }

  // Kennzahlen eines Profils; gezählt werden nur Läufe mit seinen aktuellen Limits
  private profile(name: string, maxTurns: number, timeoutMinutes: number, work: DispatchRun[], escalated: Map<string, Set<string>>): ProfileFacts {
    const all = work.filter((r) => r.profile === name);
    const current = all.filter((r) => r.maxTurns === maxTurns && r.timeoutMinutes === timeoutMinutes);
    const starts = current.filter(isStart);
    const away = escalated.get(name) ?? new Set<string>();
    const via = current.filter((r) => r.escalatedFrom !== null);
    const aborts: Record<string, number> = {};
    for (const r of current) if (r.abortCategory) aborts[r.abortCategory] = (aborts[r.abortCategory] ?? 0) + 1;
    const turns = current.map((r) => r.numTurns).filter((t): t is number => t !== null).sort((a, b) => a - b);
    return {
      profile: name, maxTurns, timeoutMinutes,
      runs: current.length,
      starts: starts.length,
      escalatedStarts: starts.filter((r) => away.has(r.subjectId)).length,
      viaEscalation: via.length,
      topOrigin: this.top(via.map((r) => r.escalatedFrom as string)),
      aborts,
      firstTrySuccess: starts.filter((r) => SUCCESS.has(r.outcome)).length,
      turnsP90: turns.length ? turns[Math.ceil(turns.length * 0.9) - 1] ?? null : null,
      startSources: this.sources(starts),
      // Läufe kommen nach Start sortiert
      lastRunAt: all.at(-1)?.startedAt ?? null,
    };
  }

  // Herkunft der Starts (Grund und Regel), häufigste zuerst
  private sources(starts: DispatchRun[]): StartSource[] {
    const map = new Map<string, StartSource>();
    for (const r of starts) {
      const key = `${r.profileReason}|${r.ruleIndex}`;
      const entry = map.get(key) ?? { reason: r.profileReason, ruleIndex: r.ruleIndex, count: 0 };
      entry.count++;
      map.set(key, entry);
    }
    return [...map.values()].sort((a, b) => b.count - a.count || String(a.reason).localeCompare(String(b.reason)));
  }

  // Starts je Regel – nur Läufe, deren Konfig-Fassung an dieser Stelle dieselbe Regel hatte
  private rules(input: FactsInput, work: DispatchRun[], escalated: Map<string, Set<string>>): RuleFacts[] {
    const rulesOf = new Map<string, unknown[]>();
    for (const v of input.versions) {
      try {
        const parsed = JSON.parse(v.content) as { rules?: unknown };
        rulesOf.set(v.hash, Array.isArray(parsed.rules) ? parsed.rules : []);
      } catch {
        rulesOf.set(v.hash, []);
      }
    }
    return input.config.rules.map((rule, i) => {
      const key = sortedJson(rule);
      const sameRule = (hash: string | null) => {
        const old = hash === null ? undefined : rulesOf.get(hash)?.[i];
        return old !== undefined && sortedJson(old) === key;
      };
      const runs = work.filter((r) => r.profileReason === 'rule' && r.ruleIndex === i + 1 && isStart(r) && sameRule(r.configHash));
      const failed = runs.filter((r) => r.outcome === 'abort-final' || (escalated.get(r.profile)?.has(r.subjectId) ?? false)).length;
      return { index: i + 1, profile: rule.profile, runs: runs.length, failed };
    });
  }

  // Eskalationswege Basisprofil → Zielprofil, häufigste zuerst
  private paths(work: DispatchRun[]): EscalationPath[] {
    const map = new Map<string, EscalationPath>();
    for (const r of work) {
      if (r.escalatedFrom === null) continue;
      const key = `${r.escalatedFrom}>${r.profile}`;
      const entry = map.get(key) ?? { from: r.escalatedFrom, to: r.profile, count: 0 };
      entry.count++;
      map.set(key, entry);
    }
    return [...map.values()].sort((a, b) => b.count - a.count || a.from.localeCompare(b.from));
  }

  // Häufigster Wert; bei Gleichstand alphabetisch der erste
  private top(values: string[]): string | null {
    const counts = new Map<string, number>();
    for (const v of values) counts.set(v, (counts.get(v) ?? 0) + 1);
    return [...counts.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))[0]?.[0] ?? null;
  }
}
