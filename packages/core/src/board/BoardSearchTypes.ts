// Suche über alle Boards: Anfrage an das Repository, gefundene Zeilen und Treffer für die Oberfläche.

export const SEARCH_KINDS = ['ticket', 'question', 'note'] as const;
export type SearchKind = (typeof SEARCH_KINDS)[number];

export type SearchField =
  | 'title' | 'description' | 'comment' | 'plan' | 'handbook' | 'acceptance' | 'decision' | 'question-comment' | 'note';

// Grenzen der Eingabe und der Treffer
export const SEARCH_MIN_LENGTH = 2;
export const SEARCH_MAX_LENGTH = 100;
export const SEARCH_MAX_TERMS = 5;
export const SEARCH_PER_SOURCE = 30;
export const SEARCH_MAX_LIMIT = 100;
export const SEARCH_DEFAULT_LIMIT = 50;
// Zeichen vor und nach der Fundstelle, die das Repository als Ausschnitt liefert
export const SEARCH_WINDOW = 150;

// Anfrage an das Repository: alle Begriffe müssen im selben Feld vorkommen (Groß/Klein egal)
export interface SearchRequest {
  terms: string[];
  kinds: SearchKind[];
  // Höchstens so viele Zeilen je Quelle (Tabelle und Feld)
  perSource: number;
}

// Gefundene Zeile: ein Feld eines Tickets, einer Frage oder einer Notiz
export interface SearchRow {
  kind: SearchKind;
  id: string;
  // Ticket zum Öffnen (Ticket, Kommentar, übernommene Notiz); sonst null
  ticketId: string | null;
  field: SearchField;
  title: string;
  projectId: number;
  // Fragen gehören zu keinem Board
  boardId: number | null;
  status: string;
  // Ausschnitt des Feldes um die erste Fundstelle (oder das ganze Feld)
  text: string;
  // Position des Ausschnitts im Feld (0 = Anfang) und Länge des ganzen Feldes
  textStart: number;
  textLength: number;
  at: string | null;
}

export interface SearchRows {
  rows: SearchRow[];
  // Mindestens eine Quelle hat ihre Höchstzahl erreicht
  truncated: boolean;
}

export interface SearchHit {
  kind: SearchKind;
  id: string;
  ticketId: string | null;
  field: SearchField;
  title: string;
  project: { id: number; key: string; name: string };
  board: { id: number; name: string } | null;
  status: string;
  snippet: string;
  // Hervorzuhebende Bereiche im Ausschnitt [Anfang, Ende) als Zeichenpositionen
  matches: Array<[number, number]>;
  at: string | null;
}

export interface SearchResult {
  query: string;
  terms: string[];
  hits: SearchHit[];
  truncated: boolean;
}
