// Fachobjekte des Boards: Projekte, Boards, Tickets, Kommentare, Verlauf, Notizen.

export const TICKET_STATUSES = ['backlog', 'in_planning', 'planned', 'in_progress', 'blocked', 'done', 'not_needed'] as const;
export type TicketStatus = (typeof TICKET_STATUSES)[number];

// Spaltentitel in der Reihenfolge der Anzeige; „Wartet“ ist abgeleitet und steht nach dem Backlog
export const STATUS_LABELS: Record<TicketStatus, string> = {
  backlog: 'Backlog',
  in_planning: 'In Planung',
  planned: 'Geplant',
  in_progress: 'In Arbeit',
  blocked: 'Blockiert',
  done: 'Erledigt',
  not_needed: 'Nicht mehr nötig',
};
export const WAITING_LABEL = 'Wartet';

export const PRIORITIES = ['low', 'normal', 'high', 'urgent'] as const;
export type Priority = (typeof PRIORITIES)[number];
// Kleinere Zahl steht weiter oben
export const PRIORITY_RANK: Record<Priority, number> = { urgent: 0, high: 1, normal: 2, low: 3 };

export const NOTE_STATES = ['open', 'taken', 'dropped'] as const;
export type NoteState = (typeof NOTE_STATES)[number];

// Dauer, nach der ein Planungs-Claim verfällt
export const PLANNING_CLAIM_TTL_MS = 15 * 60 * 1000;

// Höchstlängen der Texte (passen zu den Spalten in board.*); länger ergibt „value-too-long“
export const MAX_TITLE_LENGTH = 400;
export const MAX_BLOCKED_REASON_LENGTH = 500;

// Vorsilbe des Blockiert-Grunds „fertig, ein Mensch muss es noch ansehen“; nur so blockierte Tickets lassen sich abnehmen
export const HUMAN_INTERACTION_PREFIX = 'Human Interaction:';

// Wartet das Ticket mit diesem Blockiert-Grund auf die Abnahme durch einen Menschen? („Human Interaction“ ohne Doppelpunkt zählt mit)
export function isHumanInteraction(reason: string | null | undefined): boolean {
  const text = (reason ?? '').trim();
  return text.startsWith(HUMAN_INTERACTION_PREFIX) || text === HUMAN_INTERACTION_PREFIX.slice(0, -1);
}
// Namen von Handelnden, Zuständigen und Agenten
export const MAX_PERSON_NAME_LENGTH = 100;
export const MAX_LABEL_LENGTH = 100;
export const MAX_PROJECT_NAME_LENGTH = 200;
export const MAX_BOARD_NAME_LENGTH = 200;
export const MAX_NOTE_REASON_LENGTH = 1000;
// Labels einer Notiz, kommagetrennt gespeichert
export const MAX_NOTE_LABELS_LENGTH = 1000;
// Titel einer Notiz: eine Zeile, die ohne Kontext verständlich ist
export const MAX_NOTE_TITLE_LENGTH = 200;
// Tickets, bei denen eine Notiz erneut aufgefallen ist, kommagetrennt gespeichert
export const MAX_NOTE_SEEN_IN_LENGTH = 1000;
// Spec-Ordner (z. B. 001-login), Aufgabennummer (T012), Branch und Worktree-Pfad am Ticket
export const MAX_SPEC_LENGTH = 200;
export const MAX_SPEC_TASK_LENGTH = 20;
export const MAX_BRANCH_LENGTH = 400;
export const MAX_WORKTREE_LENGTH = 500;
// Text eines Arbeitsschritts und Kennung einer Claude-Session
export const MAX_STEP_TEXT_LENGTH = 1000;
export const MAX_SESSION_ID_LENGTH = 100;

// Ein abhakbarer Arbeitsschritt eines Tickets
export interface TicketStep {
  text: string;
  done: boolean;
  /** Ausführliche Anleitung zum Schritt als Markdown (Code, Befehl, erwartetes Ergebnis) */
  detail?: string;
}

// Erledigte und alle Schritte eines Tickets
export interface StepProgress {
  done: number;
  total: number;
}

export interface BoardUser {
  id: string;
  name: string;
}

// Wer handelt: Benutzer-GUID aus der Konfig und der Name im Verlauf (z. B. „claude“)
export interface BoardActor {
  userId: string;
  name: string;
}

export interface Project {
  id: number;
  key: string;
  name: string;
  nextTicketNumber: number;
  nextQuestionNumber: number;
  createdAt: string;
  createdBy: string;
}

export interface Board {
  id: number;
  projectId: number;
  ownerId: string;
  name: string;
  repoPath: string | null;
  createdAt: string;
}

export interface Ticket {
  id: string;
  projectId: number;
  boardId: number;
  number: number;
  title: string;
  description: string;
  status: TicketStatus;
  priority: Priority;
  labels: string[];
  dependsOn: string[];
  sortOrder: number;
  assignee: string | null;
  needsPlanning: boolean;
  acceptance: string;
  plan: string;
  handbook: string;
  diagram: string;
  /** Spec-Dokument, aus dem das Ticket stammt (Pfad unter docs/superpowers); `specTask` ist historisch und ungenutzt */
  spec: string | null;
  specTask: string | null;
  /** Branch und Worktree, in dem das Ticket umgesetzt wird (harness board start) */
  branch: string | null;
  worktree: string | null;
  /** Branch, in den das Ticket am Ende gemergt wird; null = Standardbranch des Projekts. Nur vor dem Start änderbar */
  targetBranch: string | null;
  /** Betroffene Bereiche (Pfade) sowie genutzte und gelieferte Schnittstellen */
  touches: string[];
  consumes: string[];
  produces: string[];
  blockedReason: string | null;
  plannedAt: string | null;
  plannedBy: string | null;
  planningClaimBy: string | null;
  planningClaimAt: string | null;
  createdAt: string;
  createdById: string;
  createdByName: string;
  updatedAt: string;
  rev: number;
}

export interface Comment {
  id: number;
  ticketId: string;
  authorId: string;
  authorName: string;
  body: string;
  createdAt: string;
  /** Wann der Kommentar in eine Session eingespeist wurde; null = noch nicht */
  readByAgentAt: string | null;
}

export interface HistoryEntry {
  ticketId: string;
  at: string;
  byId: string;
  byName: string;
  field: string;
  from: string | null;
  to: string | null;
  note: string | null;
}

export interface Note {
  id: number;
  boardId: number;
  authorId: string;
  authorName: string;
  /** Was auffällt, als ganzer Satz; leer bei Notizen von vor der Trennung in Titel und Inhalt */
  title: string;
  /** Details: wo, woran man es sieht, vermutete Ursache, Vorschlag (darf leer sein) */
  body: string;
  source: string | null;
  labels: string[];
  /** Wie oft der Befund aufgefallen ist (1 = nur beim Anlegen) */
  seenCount: number;
  /** Weitere Tickets, bei denen er erneut aufgefallen ist (ohne das Herkunftsticket) */
  seenIn: string[];
  state: NoteState;
  ticketId: string | null;
  reason: string | null;
  createdAt: string;
}

export interface Deletion {
  at: string;
  byId: string;
  byName: string;
  ticketId: string;
  ticketTitle: string;
  fromStatus: TicketStatus | null;
}
