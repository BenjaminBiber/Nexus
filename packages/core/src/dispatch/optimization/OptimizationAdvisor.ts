import { createHash } from 'node:crypto';
import type { DispatchConfig } from '../DispatchConfig.js';
import {
  MIN_RUNS, type AdvisorResult, type ConfigPatchOp, type InsufficientData, type OptimizationFacts, type ProfileFacts, type RuleFacts,
  type Suggestion, type SuggestionKind,
} from './SuggestionTypes.js';

const TURNS_RATE = 0.2;
const TIMEOUT_RATE = 0.2;
const CONTEXT_RATE = 0.15;
const OVERTAKEN_RATE = 0.5;
const RULE_FAIL_RATE = 0.5;
const OVERSIZED_SUCCESS = 0.9;
const OVERSIZED_TURNS = 0.4;
const UNUSED_DAYS = 30;
const DAY_MS = 86_400_000;

// Kurze, stabile ID eines Vorschlags aus seinem Fingerabdruck
export function suggestionId(fingerprint: string): string {
  return createHash('sha256').update(fingerprint).digest('hex').slice(0, 16);
}

const pct = (rate: number) => `${Math.round(rate * 100)} %`;
// 1,5-fach, auf 5 aufgerundet
const raise = (value: number) => Math.ceil((value * 1.5) / 5) * 5;
const limits = (f: ProfileFacts) => `maxTurns=${f.maxTurns},timeoutMinutes=${f.timeoutMinutes}`;

/** Vorschlag vor ID und Gewicht; `values` geht bei Hinweisen in den Fingerabdruck */
interface Draft {
  kind: SuggestionKind;
  title: string;
  reason: string;
  target: Suggestion['target'];
  patch: ConfigPatchOp[] | null;
  evidence: Suggestion['evidence'];
  values: string;
}

// Fester Regelkatalog der Optimierungs-Vorschläge: Kennzahlen je Profil und Regel → Vorschläge mit Begründung und Änderung.
export class OptimizationAdvisor {
  advise(facts: OptimizationFacts, config: DispatchConfig, now: Date): AdvisorResult {
    const used = this.used(config);
    const drafts: Draft[] = [];
    const insufficient: InsufficientData[] = [];
    for (const f of facts.profiles) {
      drafts.push(...this.unused(f, used, now));
      if (f.runs < MIN_RUNS) {
        if (used.has(f.profile)) insufficient.push({ profile: f.profile, runs: f.runs, needed: MIN_RUNS });
        continue;
      }
      drafts.push(...this.limits(f), ...this.overtaken(f, config), ...this.dominates(f, facts, config), ...this.oversized(f, config));
    }
    for (const r of facts.rules) drafts.push(...this.ruleFails(r, config));
    return { suggestions: OptimizationAdvisor.sort(this.dedupe(drafts.map((d) => this.finish(d)))), insufficient };
  }

  // Übernehmbare zuerst, dann nach Gewicht, dann nach Titel
  static sort(list: Suggestion[]): Suggestion[] {
    return [...list].sort((a, b) => Number(b.patch !== null) - Number(a.patch !== null) || b.weight - a.weight || a.title.localeCompare(b.title));
  }

  // Profile, die der Dispatcher von sich aus wählen kann; research und planning gelten als genutzt, weil sie ohne Einstellung greifen
  private used(config: DispatchConfig): Set<string> {
    return new Set([
      config.defaultProfile, ...config.ladder, ...config.rules.map((r) => r.profile), config.research.profile, config.planning.profile,
      'research', 'planning',
    ].filter((p): p is string => typeof p === 'string' && p !== ''));
  }

  private limits(f: ProfileFacts): Draft[] {
    const out: Draft[] = [];
    const turns = f.aborts['max-turns'] ?? 0;
    if (turns / f.runs >= TURNS_RATE) {
      out.push({
        kind: 'turns-too-low', title: `Schritte zu knapp · ${f.profile}`,
        reason: `${turns} von ${f.runs} Läufen (${pct(turns / f.runs)}) mit „${f.profile}“ brachen ab, weil ${f.maxTurns} Schritte nicht reichten.`,
        target: { profile: f.profile }, patch: [{ path: `profiles.${f.profile}.maxTurns`, from: f.maxTurns, to: raise(f.maxTurns) }],
        evidence: { runs: f.runs, hits: turns, rate: turns / f.runs }, values: limits(f),
      });
    }
    const timeouts = f.aborts.timeout ?? 0;
    if (timeouts / f.runs >= TIMEOUT_RATE) {
      out.push({
        kind: 'timeout-too-low', title: `Zeitlimit zu knapp · ${f.profile}`,
        reason: `${timeouts} von ${f.runs} Läufen (${pct(timeouts / f.runs)}) mit „${f.profile}“ brachen nach ${f.timeoutMinutes} Minuten am Zeitlimit ab.`,
        target: { profile: f.profile }, patch: [{ path: `profiles.${f.profile}.timeoutMinutes`, from: f.timeoutMinutes, to: raise(f.timeoutMinutes) }],
        evidence: { runs: f.runs, hits: timeouts, rate: timeouts / f.runs }, values: limits(f),
      });
    }
    const context = f.aborts.context ?? 0;
    if (context / f.runs >= CONTEXT_RATE) {
      out.push({
        kind: 'context-full', title: `Kontext läuft voll · ${f.profile}`,
        reason: `${context} von ${f.runs} Läufen (${pct(context / f.runs)}) mit „${f.profile}“ liefen in die Grenze des Kontextfensters. ` +
          'Ein Modell mit größerem Kontext (z. B. „opus[1m]“) oder kleiner geschnittene Tickets helfen.',
        target: { profile: f.profile }, patch: null, evidence: { runs: f.runs, hits: context, rate: context / f.runs }, values: limits(f),
      });
    }
    return out;
  }

  // Nächste Stufe der Leiter; null an der Spitze oder außerhalb der Leiter
  private next(config: DispatchConfig, profile: string): string | null {
    const i = config.ladder.indexOf(profile);
    return i >= 0 && i < config.ladder.length - 1 ? config.ladder[i + 1] ?? null : null;
  }

  // Änderung, die die Wahl von `from` auf `to` umlenkt: über die Regel, aus der seine Starts meist kommen, sonst über das Standardprofil
  private redirect(f: ProfileFacts, from: string, to: string, config: DispatchConfig): { patch: ConfigPatchOp[]; ruleIndex?: number } | null {
    const source = f.startSources[0];
    if (source?.reason === 'rule' && source.ruleIndex !== null) {
      const i = source.ruleIndex - 1;
      if (config.rules[i]?.profile !== from) return null;
      return { patch: [{ path: `rules.${i}.profile`, from, to }], ruleIndex: source.ruleIndex };
    }
    if (config.defaultProfile === from) return { patch: [{ path: 'defaultProfile', from, to }] };
    return null;
  }

  private overtaken(f: ProfileFacts, config: DispatchConfig): Draft[] {
    if (f.starts < MIN_RUNS) return [];
    const rate = f.escalatedStarts / f.starts;
    if (rate < OVERTAKEN_RATE) return [];
    const next = this.next(config, f.profile);
    const way = next ? this.redirect(f, f.profile, next, config) : null;
    let reason = `${f.escalatedStarts} von ${f.starts} Tickets (${pct(rate)}), die mit „${f.profile}“ starteten, mussten eskaliert werden.`;
    if (!next) reason += ` Über „${f.profile}“ gibt es keine höhere Stufe in der Leiter.`;
    else if (way?.ruleIndex) reason += ` Sie kamen meist über Regel ${way.ruleIndex} – sie könnte gleich „${next}“ wählen.`;
    else if (way) reason += ` „${f.profile}“ ist das Standardprofil – „${next}“ als Standard spart die Umwege.`;
    return [{
      kind: 'profile-overtaken', title: `Profil wird meist überholt · ${f.profile}`, reason,
      target: { profile: f.profile, ...(way?.ruleIndex ? { ruleIndex: way.ruleIndex } : {}) }, patch: way?.patch ?? null,
      evidence: { runs: f.starts, hits: f.escalatedStarts, rate }, values: limits(f),
    }];
  }

  private dominates(f: ProfileFacts, facts: OptimizationFacts, config: DispatchConfig): Draft[] {
    if (f.viaEscalation < MIN_RUNS || f.viaEscalation <= f.starts || !f.topOrigin) return [];
    const origin = facts.profiles.find((p) => p.profile === f.topOrigin);
    const way = origin ? this.redirect(origin, origin.profile, f.profile, config) : null;
    const rate = f.viaEscalation / (f.viaEscalation + f.starts);
    let reason = `„${f.profile}“ wurde ${f.viaEscalation}-mal per Eskalation erreicht, aber nur ${f.starts}-mal direkt gewählt – meist von „${f.topOrigin}“ aus.`;
    if (way?.ruleIndex) reason += ` Regel ${way.ruleIndex} könnte gleich „${f.profile}“ wählen.`;
    else if (way) reason += ` „${f.profile}“ als Standardprofil spart die Umwege.`;
    return [{
      kind: 'escalation-target-dominates', title: `Eskalationsziel öfter genutzt als direkt gewählt · ${f.profile}`, reason,
      target: { profile: f.profile, ...(way?.ruleIndex ? { ruleIndex: way.ruleIndex } : {}) }, patch: way?.patch ?? null,
      evidence: { runs: f.viaEscalation + f.starts, hits: f.viaEscalation, rate }, values: `origin=${f.topOrigin}`,
    }];
  }

  private oversized(f: ProfileFacts, config: DispatchConfig): Draft[] {
    const i = config.ladder.indexOf(f.profile);
    if (i <= 0 || f.starts < MIN_RUNS || f.turnsP90 === null) return [];
    const rate = f.firstTrySuccess / f.starts;
    if (rate < OVERSIZED_SUCCESS || f.turnsP90 > f.maxTurns * OVERSIZED_TURNS) return [];
    return [{
      kind: 'profile-oversized', title: `Profil eventuell zu groß · ${f.profile}`,
      reason: `${pct(rate)} der Tickets mit „${f.profile}“ gelingen im ersten Versuch, 90 % der Läufe brauchen höchstens ${f.turnsP90} von ` +
        `${f.maxTurns} Schritten. Ein Versuch mit „${config.ladder[i - 1]}“ oder weniger Schritten könnte Kosten sparen.`,
      target: { profile: f.profile }, patch: null, evidence: { runs: f.starts, hits: f.firstTrySuccess, rate }, values: limits(f),
    }];
  }

  private unused(f: ProfileFacts, used: Set<string>, now: Date): Draft[] {
    if (used.has(f.profile)) return [];
    if (f.lastRunAt !== null && now.getTime() - new Date(f.lastRunAt).getTime() < UNUSED_DAYS * DAY_MS) return [];
    return [{
      kind: 'profile-unused', title: `Profil ungenutzt · ${f.profile}`,
      reason: `„${f.profile}“ steht weder in der Leiter noch in einer Regel und lief seit über ${UNUSED_DAYS} Tagen nicht. Es kann entfernt werden.`,
      target: { profile: f.profile }, patch: null, evidence: null, values: '',
    }];
  }

  private ruleFails(r: RuleFacts, config: DispatchConfig): Draft[] {
    if (r.runs < MIN_RUNS) return [];
    const rate = r.failed / r.runs;
    if (rate < RULE_FAIL_RATE) return [];
    const next = this.next(config, r.profile);
    return [{
      kind: 'rule-fails', title: `Regel ${r.index} führt in Abbrüche`,
      reason: `${r.failed} von ${r.runs} Tickets (${pct(rate)}) aus Regel ${r.index} („${r.profile}“) wurden eskaliert oder scheiterten endgültig.` +
        (next ? ` Die nächste Stufe „${next}“ wäre ein besserer Start.` : ` Über „${r.profile}“ gibt es keine höhere Stufe in der Leiter.`),
      target: { profile: r.profile, ruleIndex: r.index },
      patch: next ? [{ path: `rules.${r.index - 1}.profile`, from: r.profile, to: next }] : null,
      evidence: { runs: r.runs, hits: r.failed, rate }, values: `profile=${r.profile}`,
    }];
  }

  // Fingerabdruck aus Regel, Ziel und betroffenen Werten; Gewicht = betroffene Läufe × Quote
  private finish(d: Draft): Suggestion {
    const target = `${d.target.profile ?? ''}${d.target.ruleIndex ? `#${d.target.ruleIndex}` : ''}`;
    const values = d.patch ? d.patch.map((op) => `${op.path}=${JSON.stringify(op.from)}`).join(',') : d.values;
    const fingerprint = `rule:${d.kind}:${target}:${values}`;
    return {
      id: suggestionId(fingerprint), fingerprint, source: 'rule', kind: d.kind, title: d.title, reason: d.reason, target: d.target,
      patch: d.patch, weight: d.evidence ? d.evidence.hits * d.evidence.rate : 0, evidence: d.evidence,
    };
  }

  // Gleiche Änderung aus zwei Regeln (z. B. „überholt“ und „Eskalationsziel“): nur die gewichtigere bleibt
  private dedupe(list: Suggestion[]): Suggestion[] {
    const hints = list.filter((s) => !s.patch);
    const best = new Map<string, Suggestion>();
    for (const s of list) {
      if (!s.patch) continue;
      const key = JSON.stringify(s.patch);
      const prev = best.get(key);
      if (!prev || s.weight > prev.weight) best.set(key, s);
    }
    return [...best.values(), ...hints];
  }
}
