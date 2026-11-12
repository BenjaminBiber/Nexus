// Typen der Dispatcher-Routen (/api/dispatch), wie sie der Server liefert.
import type { AutopilotSettings } from './generated/dispatch.js';

export interface DispatchProfileDto {
  model?: string | null;
  effort?: string | null;
  maxTurns?: number;
  timeoutMinutes?: number;
  /** Claude-Accounts, die das Profil nutzen darf; fehlt = jeder aktive */
  accounts?: string[];
}

export interface DispatchRuleDto {
  when?: { labels?: string[]; priority?: string[]; filesMin?: number; planMinChars?: number };
  profile?: string;
  /** Überschreibt die Accounts des Profils; fehlt = wie das Profil */
  accounts?: string[];
}

// Inhalt von ~/.nexus/dispatch.yaml (alles optional)
export interface RawDispatchConfigDto {
  enabled?: boolean;
  boards?: Array<number | string> | null;
  wip?: number;
  window?: { from?: string; to?: string } | null;
  optInLabel?: string | null;
  optOutLabel?: string | null;
  agentPrefix?: string;
  dispatcherName?: string;
  staleMinutes?: number | null;
  maxAttempts?: number;
  pauseMinutesOnLimit?: number;
  promptExtra?: string;
  /** Eigene Vorlagen je Laufart; leer oder null = Standardvorlage */
  prompts?: { work?: string | null; plan?: string | null; research?: string | null; merge?: string | null } | null;
  /** Zurückmergen: Merge-Commit oder Squash (ein Commit je Ticket-Branch) */
  mergeStrategy?: 'merge' | 'squash';
  /** Vor dem Merge auf Freigabe warten (fehlend = true) */
  reviewBeforeMerge?: boolean;
  /** Worktrees: einer je Ticket (none) oder Tickets einer Spec gebündelt (sameSpec, alt) */
  joinWorktrees?: 'none' | 'sameSpec';
  /** Einrichtung neu angelegter Worktrees (cwd relativ zum Worktree) und Zeitlimit je Befehl in Minuten */
  worktreeSetup?: Array<{ cwd?: string; run?: string }> | null;
  worktreeSetupTimeoutMinutes?: number;
  claude?: {
    command?: string; model?: string | null; effort?: string | null; maxTurns?: number; timeoutMinutes?: number;
    permissionMode?: string | null; allowedTools?: string[]; useSubscription?: boolean; extraArgs?: string[]; gitBashPath?: string | null;
  };
  profiles?: Record<string, DispatchProfileDto>;
  defaultProfile?: string;
  rules?: DispatchRuleDto[];
  escalate?: boolean;
  escalation?: string[];
  allowFable?: boolean;
  /** Wahl unter mehreren Accounts: meiste freie Kapazität oder erster freier der Liste */
  accountStrategy?: 'most-free' | 'order';
  /** Ab dieser Auslastung (Prozent) gilt ein Account als voll */
  accountLimitPercent?: number;
  /** Erwarteter Verbrauch eines Laufs im 5-Stunden-Fenster (Prozentpunkte), solange Schätzdaten fehlen */
  accountReserveFiveHour?: number;
  /** Erwarteter Verbrauch eines Laufs in der Woche (Prozentpunkte), solange Schätzdaten fehlen */
  accountReserveWeek?: number;
  /** Setzt sich das 5-Stunden-Fenster in so vielen Minuten zurück, entfällt dessen Reserve */
  accountResetGraceMinutes?: number;
  research?: { auto?: boolean; wip?: number; maxPerDay?: number; profile?: string | null } | null;
  planning?: { auto?: boolean; wip?: number; acceptance?: boolean; profile?: string | null } | null;
  /** Autopilot: keine Freigaben, Fragen entscheidet die Recherche, Merge-Konflikte mehrfach */
  autopilot?: Partial<AutopilotSettings> | null;
}

export type DispatchPromptKindDto = 'work' | 'plan' | 'research' | 'merge';

export interface DispatchPromptPlaceholderDto {
  name: string;
  description: string;
}

/** Standardvorlage, Platzhalter und Pflichtteile einer Laufart – zum Bearbeiten der Vorlagen */
export interface DispatchPromptInfoDto {
  kind: DispatchPromptKindDto;
  label: string;
  default: string;
  placeholders: DispatchPromptPlaceholderDto[];
  required: string[];
}

/** Ergebnis der Vorschau; `sample`: mit Beispielwerten gerendert, nicht mit denen eines Tickets */
export interface DispatchPromptPreviewDto {
  kind: DispatchPromptKindDto;
  id: string | null;
  sample: boolean;
  prompt: string;
}

export interface DispatchConfigViewDto {
  file: string;
  exists: boolean;
  raw: RawDispatchConfigDto;
  config: { enabled: boolean; staleMinutes: number; ladder: string[] } | null;
  error: string | null;
  /** Vorlagen-Angaben je Laufart; ältere Server liefern das Feld nicht */
  promptInfo?: DispatchPromptInfoDto[];
}

export type DispatchStateKind = 'off' | 'idle' | 'running' | 'paused' | 'outside' | 'error';

export interface DispatchRunDto {
  id: string;
  kind: 'work' | 'merge' | 'research' | 'plan';
  /** Ticket; bei einer Recherche die Frage */
  ticketId: string;
  /** Recherche: ID der Frage */
  questionId?: string;
  title: string;
  profile: string;
  model: string | null;
  effort: string | null;
  startedAt: string;
  log: string;
  /** Claude-Account des Laufs; fehlt ohne Account-Wahl */
  account?: string;
}

/** Lauf eines Tickets oder einer Frage aus den Protokollen (Reiter „Agent“), neueste zuerst */
export interface DispatchRunLogDto {
  /** Dateiname des Protokolls */
  name: string;
  kind: 'work' | 'merge' | 'research' | 'plan';
  /** Start (ISO) laut Dateiname */
  started: string | null;
  /** Lauf noch nicht beendet */
  running: boolean;
  /** Ergebnis des Laufs aus der Board-Datenbank (`done`, `abort`, `review`, …); fehlt bei laufenden und unbekannten Läufen */
  outcome?: string | null;
  /** Grund eines Abbruchs */
  abortReason?: string | null;
  /** Ende des Laufs (ISO) */
  endedAt?: string | null;
  /** Dauer in Millisekunden */
  durationMs?: number | null;
  /** Modell des Laufs, z. B. `claude-opus-5-5` */
  model?: string | null;
}

/** Ein Screenshot, den der Agent in einem Lauf gemacht hat */
export interface DispatchRunShotDto {
  /** Dateiname in der Ablage des Laufs, z. B. `001.png` */
  file: string;
  /** ID des Tool-Aufrufs, zu dem das Bild gehört */
  toolUseId: string;
  /** Zeitpunkt der Ablage (ISO) */
  at: string;
  mediaType: string;
  bytes: number;
}

/** Lauf-Protokoll (Ende der Datei), zerlegt in Kopf, Arbeitsanweisung und Ausgabe */
export interface DispatchRunLogPartsDto {
  name: string;
  header: string;
  /** null: Anweisung steht nicht im gelesenen Teil */
  prompt: string | null;
  body: string;
}

export interface DispatchMergeDto {
  ticketId: string;
  phase: 'pending' | 'waiting' | 'session';
  branch: string;
  /** Zielbranch des Tickets; null ohne Zielbranch (ältere Server liefern das Feld nicht) */
  target?: string | null;
}

export interface DispatchTicketRowDto {
  id: string;
  title: string;
  reason: string;
}

export interface DispatchStatusDto {
  enabled: boolean;
  state: DispatchStateKind;
  message: string;
  pausedUntil: string | null;
  /** Laufende Pausen einzelner Claude-Accounts (ältere Server liefern das Feld nicht) */
  accountPauses?: Record<string, string>;
  wip: number;
  active: number;
  /** Laufende Planungen („Planen“ im Ticket; ältere Server liefern das Feld nicht) */
  planning?: number;
  runs: DispatchRunDto[];
  /** Recherchen: Automatik, Grenze, laufende (ältere Server liefern das Feld nicht) */
  /** Recherchen: Automatik, Grenzen, laufende, heute automatisch gestartete; Fehler der Automatik und Neuversuch */
  research?: { auto: boolean; wip: number; active: number; maxPerDay?: number; today?: number; since?: string | null; retryAt?: string | null; error?: string | null };
  /** Planungs-Automatik: an, Grenzen, laufende, heute automatisch gestartete; Fehler der Automatik und Neuversuch */
  planningAuto?: { auto: boolean; wip: number; active: number; since?: string | null; retryAt?: string | null; error?: string | null };
  merges: DispatchMergeDto[];
  boardId: number | null;
  boardInScope: boolean;
  tickets: DispatchTicketRowDto[];
  ticketsError: string | null;
  lastTick: string | null;
  lastErrors: string[];
  configFile: string;
  configError: string | null;
  agentPrefix: string;
}

// Antwort von „Erneut mergen“: Zustand plus Ergebnis für das Ticket
export interface DispatchMergeResultDto extends DispatchStatusDto {
  result?: { outcome: 'merged' | 'blocked' | 'waiting' | 'session' | 'pending' | 'accepted' | 'returned'; text: string };
}

// Testlauf des Agenten (ok null = ohne Ergebnis)
export interface DispatchTestRunDto {
  command: string;
  ok: boolean | null;
  output: string;
}

// Geänderte Datei im Review
export interface DispatchReviewFileDto {
  path: string;
  oldPath?: string;
  status: string;
  additions: number;
  deletions: number;
  binary: boolean;
}

// Antwort von GET /api/dispatch/review/:id: Diff gegen das Ziel und Testläufe des Agenten
export interface DispatchReviewDto {
  ticketId: string;
  branch: string | null;
  target: string | null;
  base: string | null;
  files: DispatchReviewFileDto[];
  tests: DispatchTestRunDto[];
  testsAt: string | null;
  runLog: string | null;
  available: boolean;
  reason: string | null;
}

// Antwort von GET /api/dispatch/review/:id/file: Patch einer Datei
export interface DispatchReviewPatchDto {
  path: string;
  patch: string;
  truncated: boolean;
}

// Antwort von „Recherche starten“: Zustand plus der gestartete Lauf
export interface DispatchResearchStartDto extends DispatchStatusDto {
  started: DispatchRunDto;
}

// Antwort von GET /api/dispatch/research/:id: läuft für die Frage gerade eine Recherche?
export interface QuestionResearchStatusDto {
  questionId: string;
  run: DispatchRunDto | null;
  pausedUntil: string | null;
  lastStartedAt: string | null;
  count: number;
}

// Antwort von GET /api/dispatch/plan/:id: läuft für das Ticket gerade eine Planung?
export interface TicketPlanningStatusDto {
  ticketId: string;
  run: DispatchRunDto | null;
  pausedUntil: string | null;
}

// Art eines Laufs in der Übersicht
export const RUN_KIND_LABELS: Record<DispatchRunDto['kind'], string> = {
  work: 'Umsetzung',
  merge: 'Merge-Session',
  research: 'Recherche',
  plan: 'Planung',
};

export const EFFORT_OPTIONS = ['low', 'medium', 'high', 'xhigh', 'max'] as const;

export const MERGE_PHASE_LABELS: Record<DispatchMergeDto['phase'], string> = {
  pending: 'Merge steht an',
  waiting: 'wartet auf weitere Tickets',
  session: 'Merge-Session läuft',
};

/** Eine Änderung eines Vorschlags; Pfade wie profiles.<name>.maxTurns, defaultProfile, rules.<i>.profile, escalation */
export interface ConfigPatchOpDto {
  path: string;
  from: unknown;
  to: unknown;
}

export type SuggestionKindDto =
  | 'turns-too-low' | 'timeout-too-low' | 'profile-overtaken' | 'escalation-target-dominates' | 'rule-fails'
  | 'profile-oversized' | 'context-full' | 'profile-unused' | 'claude';

export interface SuggestionDto {
  id: string;
  fingerprint: string;
  source: 'rule' | 'claude';
  kind: SuggestionKindDto;
  title: string;
  reason: string;
  target: { profile?: string; ruleIndex?: number };
  /** null = nur Hinweis */
  patch: ConfigPatchOpDto[] | null;
  weight: number;
  evidence: { runs: number; hits: number; rate: number } | null;
}

export interface SuggestionListDto {
  configHash: string;
  basis: { runs: number; projects: number; since: string };
  suggestions: SuggestionDto[];
  dismissed: SuggestionDto[];
  insufficient: { profile: string; runs: number; needed: number }[];
  analysis: { at: string; costUsd: number | null; stale: boolean; count: number; skipped: number } | null;
}

export interface SuggestionApplyDto {
  view: DispatchConfigViewDto;
  list: SuggestionListDto;
}
