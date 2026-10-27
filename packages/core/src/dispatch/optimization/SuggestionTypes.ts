import type { DispatchProfileReason } from '../../board/DispatchRunTypes.js';

/** Herkunft eines Vorschlags: feste Regel des Advisors oder Claude-Analyse */
export type SuggestionSource = 'rule' | 'claude';

export type SuggestionKind =
  | 'turns-too-low' | 'timeout-too-low' | 'profile-overtaken' | 'escalation-target-dominates' | 'rule-fails'
  | 'profile-oversized' | 'context-full' | 'profile-unused' | 'claude';

/** Eine Änderung an der Dispatcher-Einstellung; `from` ist der wirksame Wert, auf dem der Vorschlag beruht */
export interface ConfigPatchOp {
  /** profiles.<name>.maxTurns|timeoutMinutes|model|effort, defaultProfile, rules.<index ab 0>.profile oder escalation */
  path: string;
  from: unknown;
  to: unknown;
}

export interface Suggestion {
  /** Kurzer Hash des Fingerabdrucks; Schlüssel für Übernehmen und Ausblenden */
  id: string;
  /** Regel, Ziel und betroffene Werte – ändert sich mit dem Profil, damit Ausgeblendetes nach einer Änderung wiederkommt */
  fingerprint: string;
  source: SuggestionSource;
  kind: SuggestionKind;
  title: string;
  reason: string;
  target: { profile?: string; ruleIndex?: number };
  /** null = nur Hinweis */
  patch: ConfigPatchOp[] | null;
  /** Sortierung: betroffene Läufe × Quote */
  weight: number;
  evidence: { runs: number; hits: number; rate: number } | null;
}

/** Wie die Starts eines Profils zustande kamen (Grund und Regel), häufigste zuerst */
export interface StartSource {
  reason: DispatchProfileReason | null;
  ruleIndex: number | null;
  count: number;
}

/** Kennzahlen eines Profils über die Läufe mit seinen aktuellen Limits */
export interface ProfileFacts {
  profile: string;
  maxTurns: number;
  timeoutMinutes: number;
  /** Läufe mit den aktuellen Limits */
  runs: number;
  /** davon Starts eines Tickets (ohne Eskalation, erster Versuch) */
  starts: number;
  /** Starts, deren Ticket danach von diesem Profil weg eskaliert wurde */
  escalatedStarts: number;
  /** Läufe, die per Eskalation auf dieses Profil kamen */
  viaEscalation: number;
  /** Häufigstes Basisprofil dieser Eskalationen */
  topOrigin: string | null;
  /** Abbruch-Kategorie → Anzahl */
  aborts: Record<string, number>;
  /** Starts mit Ergebnis done oder review */
  firstTrySuccess: number;
  /** 90. Perzentil der Schritte; null ohne Angaben */
  turnsP90: number | null;
  startSources: StartSource[];
  /** Letzter Lauf mit diesem Profil (egal welche Limits); null ohne Lauf im Zeitraum */
  lastRunAt: string | null;
}

/** Kennzahlen einer Regel über die Starts, die sie in ihrer aktuellen Fassung ausgelöst hat */
export interface RuleFacts {
  /** 1-basiert wie in der Oberfläche */
  index: number;
  profile: string;
  runs: number;
  /** davon eskaliert oder endgültig gescheitert */
  failed: number;
}

export interface EscalationPath {
  from: string;
  to: string;
  count: number;
}

export interface OptimizationBasis {
  /** ausgewertete Arbeitsläufe */
  runs: number;
  projects: number;
  /** Beginn des Zeitraums (ISO) */
  since: string;
}

export interface OptimizationFacts {
  basis: OptimizationBasis;
  profiles: ProfileFacts[];
  rules: RuleFacts[];
  paths: EscalationPath[];
}

export interface InsufficientData {
  profile: string;
  runs: number;
  needed: number;
}

export interface AdvisorResult {
  suggestions: Suggestion[];
  insufficient: InsufficientData[];
}

/** Mindestzahl passender Läufe, bevor eine Regel urteilt */
export const MIN_RUNS = 5;
