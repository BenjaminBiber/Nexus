import type { AuthorKind } from '../actors/Actor.js';

// Zustände einer Frage; „bereit“, „wartet auf“ und „recherchiert“ werden berechnet, nicht gespeichert
export const QUESTION_STATES = ['open', 'answered', 'discarded'] as const;
export type QuestionState = (typeof QUESTION_STATES)[number];

// Anzeigetexte der Zustände
export const QUESTION_STATE_LABELS: Record<QuestionState, string> = {
  open: 'offen',
  answered: 'beantwortet',
  discarded: 'verworfen',
};

// Arten von Bezügen einer Frage
export const LINK_TYPES = ['ticket', 'spec', 'spec-task', 'file'] as const;
export type LinkType = (typeof LINK_TYPES)[number];

// Ein Bezug auf ein Ticket, eine Spec, eine Spec-Aufgabe oder eine Datei im Code
export interface QuestionLink {
  type: LinkType;
  target: string;
}

// Wer die Frage angelegt hat; bei Sessions mit Kennung der Session
export interface QuestionOrigin {
  kind: AuthorKind;
  author: string;
  session?: string;
}

// Ein Lösungsvorschlag einer Recherche; Nummer fortlaufend je Frage (`O1`, `O2`, …)
export interface QuestionOption {
  id: string;
  title: string;
  text: string;
  pros: string[];
  cons: string[];
  recommended: boolean;
}

// Ein datierter Eintrag eines Agenten mit Kontext und Optionen
export interface Research {
  at: string;
  author: string;
  kind: AuthorKind;
  context: string;
  options: QuestionOption[];
}

// Die vom Menschen getroffene Entscheidung (oder das Verwerfen) mit Begründung
export interface Decision {
  outcome: 'answered' | 'discarded';
  option: string | null;
  text: string | null;
  reason: string;
  by: string;
  at: string;
}

export type QuestionEntryType =
  | 'comment' | 'created' | 'title' | 'tags' | 'description' | 'occasion'
  | 'links' | 'depends' | 'research' | 'decided' | 'discarded' | 'reopened';

// Ein datierter Eintrag im Verlauf; `read` gibt es nur bei Kommentaren
export interface QuestionEntry {
  at: string;
  type: QuestionEntryType;
  author: string;
  kind: AuthorKind;
  read?: boolean;
  text: string;
}

// Eine Frage mit allen Feldern aus dem Frontmatter, Beschreibung, Recherchen und Verlauf
export interface Question {
  id: string;
  title: string;
  state: QuestionState;
  tags: string[];
  origin: QuestionOrigin;
  occasion: string | null;
  links: QuestionLink[];
  depends_on: string[];
  decision: Decision | null;
  created: string;
  updated: string;
  description: string;
  research: Research[];
  history: QuestionEntry[];
}
