import type { MergePhase, RunKind, RunLogEntry } from '@harness/core';

export type DispatchStateKind = 'off' | 'idle' | 'running' | 'paused' | 'outside' | 'error';

export interface DispatchRunInfo {
  id: string;
  kind: RunKind;
  ticketId: string;
  title: string;
  profile: string;
  model: string | null;
  effort: string | null;
  startedAt: string;
  /** Dateiname des Protokolls */
  log: string;
  /** Recherche: ID der Frage */
  questionId?: string;
}

/** Lauf eines Tickets oder einer Frage aus den Protokollen (Reiter „Agent“) */
export interface DispatchRunLog extends RunLogEntry {
  /** Der Worker läuft noch: der Lauf ist eingetragen und hat noch keine Ergebnisdatei */
  running: boolean;
}

// Recherchen im Zustand: Automatik, Grenze und laufende Recherche-Läufe
export interface DispatchResearchInfo {
  auto: boolean;
  wip: number;
  active: number;
  /** Höchstens so viele automatische Recherchen am Tag, davon heute gestartet */
  maxPerDay: number;
  today: number;
  /** Automatik gilt für Fragen ab diesem Zeitpunkt (null = aus) */
  since: string | null;
  /** Nach einem Umgebungs- oder Repo-Fehler ausgesetzt bis (null = nicht ausgesetzt) */
  retryAt: string | null;
  /** Letzter Umgebungs- oder Repo-Fehler der Automatik */
  error: string | null;
}

// Planungen im Zustand: Automatik, Grenze und laufende Planungsläufe
export interface DispatchPlanningInfo {
  auto: boolean;
  wip: number;
  active: number;
  /** Automatik gilt für Tickets ab diesem Zeitpunkt (null = aus) */
  since: string | null;
  /** Nach einem Umgebungs- oder Repo-Fehler ausgesetzt bis (null = nicht ausgesetzt) */
  retryAt: string | null;
  /** Letzter Umgebungs- oder Repo-Fehler der Automatik */
  error: string | null;
}

// Antwort von GET /api/dispatch/research/:questionId: läuft für die Frage gerade eine Recherche?
export interface QuestionResearchStatus {
  questionId: string;
  run: DispatchRunInfo | null;
  /** Pause des Dispatchers (z. B. Nutzungslimit); ein Start von Hand wird solange abgelehnt */
  pausedUntil: string | null;
  lastStartedAt: string | null;
  count: number;
}

// Antwort von GET /api/dispatch/plan/:ticketId: läuft für das Ticket gerade eine Planung?
export interface TicketPlanningStatus {
  ticketId: string;
  run: DispatchRunInfo | null;
  /** Pause des Dispatchers (z. B. Nutzungslimit); ein Start von Hand wird solange abgelehnt */
  pausedUntil: string | null;
}

export interface DispatchMergeInfo {
  ticketId: string;
  phase: MergePhase;
  branch: string;
  /** Zielbranch des Tickets; null ohne Zielbranch (Merge in den ausgecheckten Branch) oder wenn das Ticket nicht lesbar ist */
  target: string | null;
}

export interface DispatchTicketRow {
  id: string;
  title: string;
  /** Warum (noch) nicht – oder „bereit“ bzw. „bereit – wartet auf freien Platz“ */
  reason: string;
}

// Antwort von GET /api/dispatch/status
export interface DispatchStatus {
  enabled: boolean;
  state: DispatchStateKind;
  message: string;
  pausedUntil: string | null;
  wip: number;
  /** Laufende Ticket-Läufe (Umsetzung und Merge-Session); Recherchen zählen unter research.active, Planungen unter planning */
  active: number;
  /** Laufende Planungsläufe („Planen“ im Ticket; zählen nicht zum WIP) */
  planning: number;
  runs: DispatchRunInfo[];
  research: DispatchResearchInfo;
  /** Zustand der Planungs-Automatik (`planning` bleibt die Zahl der laufenden Planungen) */
  planningAuto: DispatchPlanningInfo;
  merges: DispatchMergeInfo[];
  boardId: number | null;
  boardInScope: boolean;
  tickets: DispatchTicketRow[];
  ticketsError: string | null;
  lastTick: string | null;
  lastErrors: string[];
  configFile: string;
  configError: string | null;
  /** Namensvorsatz der Agenten (Karten zeigen dafür „Agent“) */
  agentPrefix: string;
}
