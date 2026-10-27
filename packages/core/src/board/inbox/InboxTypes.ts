// Einträge der persönlichen Arbeitsliste „Meine Aufgaben“: was gerade auf den Benutzer wartet, aus vorhandenen Board-Daten.

// Art eines Eintrags; die Reihenfolge ist zugleich die Dringlichkeit (merge zuerst)
export const INBOX_KINDS = ['merge', 'setup', 'question', 'accept', 'blocked', 'planning', 'note'] as const;
export type InboxKind = (typeof INBOX_KINDS)[number];

// Anzeigenamen der Arten
export const INBOX_KIND_LABELS: Record<InboxKind, string> = {
  merge: 'Merge gescheitert',
  setup: 'Einrichtung gescheitert',
  question: 'Frage',
  accept: 'Abnahme',
  blocked: 'Blockiert',
  planning: 'In Planung',
  note: 'Notiz',
};

// Arten, die aus einem Ticket entstehen
export type InboxTicketKind = Exclude<InboxKind, 'question' | 'note'>;

export interface InboxItem {
  /** Stabiler Schlüssel für die gespeicherte Reihenfolge: `ticket:AIH-0001`, `question:AIH-F0002`, `note:7` */
  key: string;
  kind: InboxKind;
  title: string;
  /** Blockiert-Grund, Anlass der Frage oder Auszug der Notiz */
  detail: string | null;
  boardId: number | null;
  boardName: string | null;
  projectKey: string;
  ticketId: string | null;
  questionId: string | null;
  noteId: number | null;
  /** Nur bei Fragen: Zahl der offenen Tickets, die auf die Frage warten */
  waitingTickets: number;
  /** Priorität des Tickets (nur bei Ticket-Einträgen), sortiert Einträge gleicher Art */
  priority: string | null;
  /** ISO-Zeitpunkt: letzte Änderung des Tickets bzw. Anlage von Frage und Notiz */
  since: string;
  /** Dringlichkeit, kleiner = dringender */
  rank: number;
}

// Liste samt Zählern je Art
export interface InboxList {
  items: InboxItem[];
  counts: Record<InboxKind, number>;
  total: number;
}

// Format gespeicherter Schlüssel (auch für spätere eigene ToDos `todo:`)
export const INBOX_KEY_PATTERN = /^(ticket|question|note|todo):[A-Za-z0-9-]+$/;
export const MAX_INBOX_KEYS = 1000;
export const MAX_INBOX_KEY_LENGTH = 60;
