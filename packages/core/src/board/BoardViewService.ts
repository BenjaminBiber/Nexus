import type { BoardDatabase } from './BoardDatabase.js';
import { STATUS_LABELS, WAITING_LABEL } from './BoardTypes.js';
import { BoardService } from './BoardService.js';
import { COLUMN_ORDER, type BoardColumn } from './TicketOrdering.js';
import { TicketService, type TicketView } from './TicketService.js';

// Lange Texte, die eine Karte nicht zeigt; die Ansicht sendet statt ihrer nur „vorhanden“ (das Detail lädt sie selbst)
type CardText = 'description' | 'acceptance' | 'plan' | 'handbook' | 'diagram';

export interface BoardCard extends Omit<TicketView, CardText> {
  commentCount: number;
  hasDescription: boolean;
  hasAcceptance: boolean;
  hasPlan: boolean;
  hasHandbook: boolean;
  hasDiagram: boolean;
}

export interface BoardViewColumn {
  id: BoardColumn;
  title: string;
  tickets: BoardCard[];
}

export interface BoardView {
  boardId: number;
  columns: BoardViewColumn[];
}

// Ansicht eines Boards für die Oberfläche: alle Spalten (auch leere) in fester Reihenfolge, Karten mit Kommentarzahl.
export class BoardViewService {
  private readonly tickets: TicketService;
  private readonly boards: BoardService;

  constructor(private readonly db: BoardDatabase, clock: () => Date) {
    this.tickets = new TicketService(db, clock);
    this.boards = new BoardService(db, clock);
  }

  // Liest das Board in einer Transaktion; unbekannt ergibt board-not-found
  get(boardId: number): Promise<BoardView> {
    return this.db.transaction(async (u) => {
      await this.boards.get(u, boardId);
      const views = this.tickets.sortViews(await this.tickets.views(u, await u.tickets.list({ boardIds: [boardId], withoutTexts: true })));
      const counts = await u.comments.countByBoard(boardId);
      return {
        boardId,
        columns: COLUMN_ORDER.map((id) => ({
          id,
          title: id === 'waiting' ? WAITING_LABEL : STATUS_LABELS[id],
          tickets: views.filter((v) => v.column === id).map((v) => BoardViewService.card(v, counts[v.id] ?? 0)),
        })),
      };
    });
  }

  private static card(view: TicketView, commentCount: number): BoardCard {
    const { description, acceptance, plan, handbook, diagram, ...rest } = view;
    const has = (text: string) => text.trim() !== '';
    return {
      ...rest, commentCount,
      hasDescription: has(description), hasAcceptance: has(acceptance), hasPlan: has(plan), hasHandbook: has(handbook), hasDiagram: has(diagram),
    };
  }
}
