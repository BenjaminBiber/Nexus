import type { BoardQuestion, BoardQuestionComment } from './BoardQuestionTypes.js';
import { BoardSearchMatch } from './BoardSearchMatch.js';
import type { SearchField, SearchKind, SearchRequest, SearchRow, SearchRows } from './BoardSearchTypes.js';
import type { Board, Comment, Note, Ticket } from './BoardTypes.js';

// Daten, die die Suche im Speicher liest
export interface SearchableState {
  boards: Board[];
  tickets: Ticket[];
  comments: Comment[];
  questions: BoardQuestion[];
  questionComments: BoardQuestionComment[];
  notes: Note[];
}

type Candidate = Omit<SearchRow, 'field' | 'text' | 'textStart' | 'textLength'>;

const TICKET_FIELDS: Array<[SearchField, (t: Ticket) => string]> = [
  ['title', (t) => t.title], ['description', (t) => t.description], ['acceptance', (t) => t.acceptance],
  ['plan', (t) => t.plan], ['handbook', (t) => t.handbook],
];

// Suche im Speicher mit derselben Bedeutung wie MssqlSearchRepository: alle Begriffe im selben Feld, Groß/Klein egal,
// je Quelle höchstens perSource Zeilen, neueste (höchste ID) zuerst; liefert das ganze Feld als Ausschnitt.
export class InMemoryBoardSearch {
  find(s: SearchableState, request: SearchRequest): SearchRows {
    const match = new BoardSearchMatch(request.terms);
    const hit = (text: string | null) => match.matches(text);
    const rows: SearchRow[] = [];
    let truncated = false;
    // Eine Quelle: Kandidaten in Reihenfolge „neueste zuerst“, Feldtext je Kandidat
    const source = <T>(items: T[], field: SearchField, text: (x: T) => string | null, row: (x: T) => Candidate) => {
      const found = items.filter((x) => hit(text(x))).slice(0, request.perSource + 1);
      if (found.length > request.perSource) truncated = true;
      for (const x of found.slice(0, request.perSource)) {
        const value = text(x)!;
        rows.push({ ...row(x), field, text: value, textStart: 0, textLength: value.length });
      }
    };
    const wants = (kind: SearchKind) => request.kinds.includes(kind);
    const ticketsById = new Map(s.tickets.map((t) => [t.id, t]));

    if (wants('ticket')) {
      const tickets = [...s.tickets].sort((a, b) => this.desc(a.id, b.id));
      const ticketRow = (t: Ticket): Candidate => ({
        kind: 'ticket', id: t.id, ticketId: t.id, title: t.title, projectId: t.projectId, boardId: t.boardId, status: t.status, at: null,
      });
      for (const [field, text] of TICKET_FIELDS) source(tickets, field, text, ticketRow);
      const comments = s.comments.filter((c) => ticketsById.has(c.ticketId)).sort((a, b) => b.id - a.id);
      source(comments, 'comment', (c) => c.body, (c) => ({ ...ticketRow(ticketsById.get(c.ticketId)!), at: c.createdAt }));
    }
    if (wants('question')) {
      const questions = [...s.questions].sort((a, b) => this.desc(a.id, b.id));
      const questionRow = (q: BoardQuestion): Candidate => ({
        kind: 'question', id: q.id, ticketId: null, title: q.title, projectId: q.projectId, boardId: null, status: q.state, at: null,
      });
      source(questions, 'title', (q) => q.title, questionRow);
      source(questions, 'description', (q) => q.description, questionRow);
      source(questions, 'decision', (q) => (q.decision ? `${q.decision.text ?? ''}\n${q.decision.reason}` : null),
        (q) => ({ ...questionRow(q), at: q.decision?.at ?? null }));
      const byId = new Map(s.questions.map((q) => [q.id, q]));
      const comments = s.questionComments.filter((c) => byId.has(c.questionId)).sort((a, b) => b.id - a.id);
      source(comments, 'question-comment', (c) => c.body, (c) => ({ ...questionRow(byId.get(c.questionId)!), at: c.createdAt }));
    }
    if (wants('note')) {
      const projects = new Map(s.boards.map((b) => [b.id, b.projectId]));
      const notes = [...s.notes].sort((a, b) => b.id - a.id);
      source(notes, 'note', (n) => `${n.title}\n${n.body}`, (n) => ({
        kind: 'note', id: String(n.id), ticketId: n.ticketId, title: n.title || n.body.slice(0, 200), projectId: projects.get(n.boardId) ?? 0,
        boardId: n.boardId, status: n.state, at: n.createdAt,
      }));
    }
    return { rows, truncated };
  }

  // Absteigend nach ID (Zahlen im Text numerisch)
  private desc(a: string, b: string): number {
    return b.localeCompare(a, undefined, { numeric: true });
  }
}
