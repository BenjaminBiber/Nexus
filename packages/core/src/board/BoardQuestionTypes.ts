// Fachobjekte der Fragen auf der Board-Datenbank: Frage, Recherche mit Optionen, Entscheidung, Kommentar, Verlauf.
import type { QuestionState } from '../questions/Question.js';
import type { AuthorKind } from '../actors/Actor.js';
import type { BoardActor, TicketStatus } from './BoardTypes.js';

// Höchstlängen der Texte (passen zu den Spalten in board.Questions* aus 002_questions.sql)
export const MAX_QUESTION_TITLE_LENGTH = 400;
export const MAX_QUESTION_OCCASION_LENGTH = 400;
export const MAX_OPTION_TITLE_LENGTH = 400;
export const MAX_SESSION_LENGTH = 100;

// Arten von Verlaufseinträgen einer Frage
export const BOARD_QUESTION_HISTORY_TYPES = [
  'created', 'title', 'description', 'tags', 'occasion', 'tickets', 'depends', 'research', 'decided', 'discarded', 'reopened', 'ticket-deleted',
] as const;
export type BoardQuestionHistoryType = (typeof BOARD_QUESTION_HISTORY_TYPES)[number];

// Wer bei Fragen handelt: Benutzer-GUID und Name wie bei Tickets, dazu die Art und bei Sessions deren Kennung
export interface BoardQuestionActor extends BoardActor {
  kind: AuthorKind;
  session: string | null;
}

// Ein Lösungsvorschlag einer Recherche; Nummer fortlaufend je Frage (O1, O2, …)
export interface BoardQuestionOption {
  id: string;
  title: string;
  text: string;
  pros: string[];
  cons: string[];
  recommended: boolean;
}

export interface BoardQuestionResearch {
  id: number;
  questionId: string;
  at: string;
  byId: string;
  byName: string;
  kind: AuthorKind;
  context: string;
  options: BoardQuestionOption[];
}

// Zahl der Recherchen einer Frage und Zahl der Optionen in der neuesten
export interface BoardQuestionResearchSummary {
  count: number;
  optionCount: number;
}

// Entscheidung eines Menschen (beantwortet oder verworfen) mit Begründung
export interface BoardQuestionDecision {
  outcome: 'answered' | 'discarded';
  option: string | null;
  text: string | null;
  reason: string;
  byId: string;
  byName: string;
  at: string;
}

export interface BoardQuestion {
  id: string;
  projectId: number;
  number: number;
  title: string;
  description: string;
  state: QuestionState;
  occasion: string | null;
  originKind: AuthorKind;
  originName: string;
  originSession: string | null;
  tags: string[];
  // Betroffene Tickets, sortiert
  ticketIds: string[];
  // Fragen oder Tickets, von denen die Klärung abhängt (Reihenfolge der Eingabe)
  dependsOn: string[];
  decision: BoardQuestionDecision | null;
  createdAt: string;
  createdById: string;
  updatedAt: string;
  rev: number;
}

export interface BoardQuestionComment {
  id: number;
  questionId: string;
  authorId: string;
  authorName: string;
  authorKind: AuthorKind;
  body: string;
  createdAt: string;
}

// byKind ist null bei Einträgen ohne eigentlichen Autor (z. B. Ticket gelöscht)
export interface BoardQuestionHistoryEntry {
  questionId: string;
  at: string;
  byId: string;
  byName: string;
  byKind: AuthorKind | null;
  type: BoardQuestionHistoryType;
  text: string;
}

// Ein betroffenes Ticket mit Titel und Status zur Anzeige
export interface BoardQuestionTicketRef {
  id: string;
  title: string;
  status: TicketStatus;
  boardId: number;
}

// Eine offene Abhängigkeit mit ihrem Zustand (Fragezustand oder Ticketstatus)
export interface BoardQuestionWaiting {
  id: string;
  state: string;
}

// Anzeigefassung einer Frage: berechnete Angaben, nicht gespeichert
export interface BoardQuestionView extends BoardQuestion {
  projectKey: string;
  tickets: BoardQuestionTicketRef[];
  waitingOn: BoardQuestionWaiting[];
  ready: boolean;
  researched: boolean;
  optionCount: number;
}

export interface BoardQuestionDetail extends BoardQuestionView {
  research: BoardQuestionResearch[];
  comments: BoardQuestionComment[];
  history: BoardQuestionHistoryEntry[];
}

// Offene Frage, auf die ein Ticket wartet (Karte, Panel, CLI)
export interface BoardOpenQuestion {
  id: string;
  title: string;
}

// Eine Frage, wie sie die Windows-Meldungen brauchen (offene eigene Fragen: selbst gestellt oder zu Tickets auf eigenen Boards)
export interface NotifiableBoardQuestion {
  id: string;
  title: string;
  occasion: string | null;
  origin: { kind: AuthorKind };
  research: Array<{ kind: AuthorKind; options: BoardQuestionOption[] }>;
  projectKey: string;
  projectName: string;
  // Eigene Repo-Pfade der Boards mit betroffenen Tickets, sonst der eigenen Boards im Projekt (für die Adresse in der Meldung)
  repoPaths: string[];
}
