import { HarnessError } from '../../errors/HarnessError.js';
import type { BoardDatabase } from '../BoardDatabase.js';
import type { Board, Note, Ticket } from '../BoardTypes.js';
import { noteTitle } from '../NoteSimilarity.js';
import { QuestionConcern } from '../QuestionConcern.js';
import { INBOX_KINDS, type InboxItem, type InboxKind, type InboxList } from './InboxTypes.js';
import { InboxRanking } from './InboxRanking.js';

const TITLE_LENGTH = 120;
const DETAIL_LENGTH = 300;

export interface InboxFilter {
  boardId?: number;
}

// Sammelt „Meine Aufgaben“: alles auf den eigenen Boards, was gerade auf den Benutzer wartet (blockierte Tickets, Tickets
// in Planung, offene Fragen nach der Regel „Eigene“, offene Notizen). Liefert die Einträge nach Dringlichkeit sortiert.
export class InboxService {
  private readonly ranking = new InboxRanking();
  private readonly concern = new QuestionConcern();

  constructor(private readonly db: BoardDatabase) {}

  list(ownerId: string, filter: InboxFilter = {}): Promise<InboxItem[]> {
    return this.db.transaction(async (u) => {
      const owned = await u.boards.list({ ownerId });
      if (filter.boardId !== undefined && !owned.some((b) => b.id === filter.boardId)) {
        throw new HarnessError('board-not-owned', `Board ${filter.boardId} gehört nicht dir.`);
      }
      if (!owned.length) return [];
      const ownIds = new Set(owned.map((b) => b.id));
      const boards = new Map(owned.map((b) => [b.id, b]));
      const projects = new Map((await u.projects.list()).map((p) => [p.id, p.key]));
      const tickets = await u.tickets.list({ boardIds: owned.map((b) => b.id) });
      const boardOf = new Map(tickets.map((t) => [t.id, t.boardId]));

      // Offene Fragen, die den Benutzer betreffen; ihre offenen Tickets erscheinen nur über die Frage
      const questions = (await u.questions.list({ states: ['open'] }))
        .filter((q) => this.concern.concerns({ createdById: q.createdById, boardIds: q.ticketIds.flatMap((t) => boardOf.get(t) ?? []) }, ownerId, ownIds));
      const referenced = [...new Set(questions.flatMap((q) => q.ticketIds))];
      const status = new Map((referenced.length ? await u.tickets.list({ ids: referenced }) : []).map((t) => [t.id, t.status]));
      const covered = new Set<string>();
      const items: InboxItem[] = [];
      for (const q of questions) {
        const open = q.ticketIds.filter((id) => { const s = status.get(id); return s !== undefined && s !== 'done' && s !== 'not_needed'; });
        open.forEach((id) => covered.add(id));
        const boardId = q.ticketIds.map((id) => boardOf.get(id)).find((id): id is number => id !== undefined) ?? null;
        if (filter.boardId !== undefined && boardId !== filter.boardId) continue;
        items.push(this.item('question', `question:${q.id}`, {
          title: q.title, detail: q.occasion, board: boardId === null ? null : boards.get(boardId)!,
          projectKey: projects.get(q.projectId) ?? '?', questionId: q.id, waitingTickets: open.length, since: q.createdAt,
        }));
      }

      for (const ticket of tickets) {
        const kind = this.ranking.classify(ticket);
        if (!kind || covered.has(ticket.id)) continue;
        if (filter.boardId !== undefined && ticket.boardId !== filter.boardId) continue;
        items.push(this.ticketItem(kind, ticket, boards.get(ticket.boardId)!, projects.get(ticket.projectId) ?? '?'));
      }

      for (const board of owned) {
        if (filter.boardId !== undefined && board.id !== filter.boardId) continue;
        for (const note of await u.notes.list({ boardId: board.id, states: ['open'] })) {
          items.push(this.noteItem(note, board, projects.get(board.projectId) ?? '?'));
        }
      }
      return this.ranking.sort(items);
    });
  }

  // Zähler je Art und gesamt
  summarize(items: InboxItem[]): InboxList {
    const counts = Object.fromEntries(INBOX_KINDS.map((k) => [k, 0])) as Record<InboxKind, number>;
    for (const item of items) counts[item.kind]++;
    return { items, counts, total: items.length };
  }

  // `since` ist die letzte Änderung des Tickets: eine Verlaufsabfrage je Ticket nur für den Zeitpunkt des Statuswechsels
  // lohnt nicht, und die Änderungszeit kommt ihm bei wartenden Tickets nahe
  private ticketItem(kind: InboxKind, ticket: Ticket, board: Board, projectKey: string): InboxItem {
    return this.item(kind, `ticket:${ticket.id}`, {
      title: ticket.title, detail: kind === 'planning' ? null : ticket.blockedReason, board, projectKey,
      ticketId: ticket.id, priority: ticket.priority, since: ticket.updatedAt,
    });
  }

  private noteItem(note: Note, board: Board, projectKey: string): InboxItem {
    // Ältere Notizen ohne Titel: erste Zeile des Inhalts als Titel, der Rest als Detail
    const lines = note.body.trim().split(/\r?\n/);
    const rest = (note.title.trim() ? lines : lines.slice(1)).join(' ').replace(/\s+/g, ' ').trim();
    const seen = note.seenCount > 1 ? `${note.seenCount}× aufgefallen` : '';
    const detail = [seen, rest].filter(Boolean).join(' · ');
    return this.item('note', `note:${note.id}`, {
      title: this.clip(noteTitle(note), TITLE_LENGTH),
      detail: detail ? this.clip(detail, DETAIL_LENGTH) : null,
      board, projectKey, noteId: note.id, since: note.createdAt,
    });
  }

  private item(kind: InboxKind, key: string, data: {
    title: string; detail: string | null; board: Board | null; projectKey: string; since: string;
    ticketId?: string | null; questionId?: string; noteId?: number; waitingTickets?: number; priority?: string;
  }): InboxItem {
    return {
      key, kind, title: data.title, detail: data.detail ? this.clip(data.detail, DETAIL_LENGTH) : null,
      boardId: data.board?.id ?? null, boardName: data.board?.name ?? null, projectKey: data.projectKey,
      ticketId: data.ticketId ?? null, questionId: data.questionId ?? null, noteId: data.noteId ?? null,
      waitingTickets: data.waitingTickets ?? 0, priority: data.priority ?? null, since: data.since, rank: this.ranking.rank(kind),
    };
  }

  private clip(text: string, max: number): string {
    return text.length > max ? `${text.slice(0, max - 1)}…` : text;
  }
}
