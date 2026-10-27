// „Meine Aufgaben“ (GET /api/db/inbox): was auf den eigenen Boards gerade auf den Benutzer wartet.

export const INBOX_KINDS = ['merge', 'setup', 'question', 'accept', 'blocked', 'planning', 'note'] as const;
export type InboxKind = (typeof INBOX_KINDS)[number];

export const INBOX_KIND_LABELS: Record<InboxKind, string> = {
  merge: 'Merge gescheitert',
  setup: 'Einrichtung gescheitert',
  question: 'Frage',
  accept: 'Abnahme',
  blocked: 'Blockiert',
  planning: 'In Planung',
  note: 'Notiz',
};

export interface InboxItemDto {
  key: string;
  kind: InboxKind;
  title: string;
  detail: string | null;
  boardId: number | null;
  boardName: string | null;
  projectKey: string;
  ticketId: string | null;
  questionId: string | null;
  noteId: number | null;
  waitingTickets: number;
  priority: string | null;
  since: string;
  rank: number;
}

export interface InboxListDto {
  items: InboxItemDto[];
  counts: Record<InboxKind, number>;
  total: number;
}
