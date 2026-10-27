import type { DatabaseSync } from 'node:sqlite';
import {
  BoardSearchMatch, type BoardSearchRepository, type SearchField, type SearchKind, type SearchRequest, type SearchRow, type SearchRows,
} from '@harness/core';
import { SqliteRunner, type SqliteValue } from './SqliteRunner.js';

// Eine Quelle: Spalten, durchsuchter Text, Tabellen samt fester Bedingung und die Sortierung „neueste zuerst“
interface Source {
  kind: SearchKind;
  field: SearchField;
  cols: string;
  /** Ausdruck des durchsuchten Textes; wird als Val geliefert und im WHERE gefiltert */
  val: string;
  from: string;
  where?: string;
  order: string;
}

const TICKET_COLS = 't.Id AS Id, t.Id AS TicketId, t.Title AS Title, t.ProjectId AS ProjectId, t.BoardId AS BoardId, t.Status AS Status';
const QUESTION_COLS = 'q.Id AS Id, NULL AS TicketId, q.Title AS Title, q.ProjectId AS ProjectId, NULL AS BoardId, q.State AS Status';

const ticketField = (field: SearchField, column: string): Source => ({
  kind: 'ticket', field, cols: `${TICKET_COLS}, NULL AS At`, val: column, from: 'Tickets t', order: 't.CreatedAt DESC, t.Id DESC',
});

// Quellen in derselben festen Reihenfolge wie MssqlSearchRepository
const SOURCES: Source[] = [
  ticketField('title', 't.Title'),
  ticketField('description', 't.Description'),
  ticketField('acceptance', 't.Acceptance'),
  ticketField('plan', 't."Plan"'),
  ticketField('handbook', 't.Handbook'),
  {
    kind: 'ticket', field: 'comment', order: 'c.Id DESC',
    cols: `${TICKET_COLS}, c.CreatedAt AS At`, val: 'c.Body', from: 'Comments c JOIN Tickets t ON t.Id = c.TicketId',
  },
  {
    kind: 'question', field: 'title', order: 'q.CreatedAt DESC, q.Id DESC',
    cols: `${QUESTION_COLS}, NULL AS At`, val: 'q.Title', from: 'Questions q',
  },
  {
    kind: 'question', field: 'description', order: 'q.CreatedAt DESC, q.Id DESC',
    cols: `${QUESTION_COLS}, NULL AS At`, val: 'q.Description', from: 'Questions q',
  },
  {
    // CONCAT behandelt NULL in SQL Server als leeren Text, || in SQLite nicht – deshalb coalesce
    kind: 'question', field: 'decision', order: 'q.CreatedAt DESC, q.Id DESC',
    cols: `${QUESTION_COLS}, q.DecidedAt AS At`, val: `coalesce(q.DecisionText, '') || char(10) || coalesce(q.DecisionReason, '')`,
    from: 'Questions q', where: 'q.DecisionOutcome IS NOT NULL',
  },
  {
    kind: 'question', field: 'question-comment', order: 'qc.Id DESC',
    cols: `${QUESTION_COLS}, qc.CreatedAt AS At`, val: 'qc.Body', from: 'QuestionComments qc JOIN Questions q ON q.Id = qc.QuestionId',
  },
  {
    kind: 'note', field: 'note', order: 'n.Id DESC',
    cols: `CAST(n.Id AS TEXT) AS Id, n.TicketId AS TicketId, CASE WHEN n.Title = '' THEN n.Body ELSE n.Title END AS Title, b.ProjectId AS ProjectId,
      n.BoardId AS BoardId, n.State AS Status, n.CreatedAt AS At`,
    val: `n.Title || char(10) || n.Body`, from: 'Notes n JOIN Boards b ON b.Id = n.BoardId',
  },
];

interface Row {
  Id: string; TicketId: string | null; Title: string; ProjectId: number; BoardId: number | null; Status: string;
  At: string | null; Val: string | null;
}

/** SQL-Funktion mit der Groß/Klein-Faltung von JavaScript (toLowerCase, Unicode) */
export const SEARCH_LOWER = 'harness_lower';

// Suche über alle Projekte mit derselben Bedeutung wie MssqlSearchRepository und der Suche im Speicher.
// SQLite faltet Groß/Klein nur für ASCII (lower, LIKE, NOCASE) – „ärger“ fände „Ärger“ also nicht. Deshalb filtert SQL
// über SEARCH_LOWER, die toLowerCase aufruft, und liefert je Quelle nur Treffer (neueste zuerst, eine Abfrage mit LIMIT);
// Ausschnitt rechnet BoardSearchMatch in JS. Die Suchbegriffe gehen nur als Parameter ins SQL.
export class SqliteSearchRepository implements BoardSearchRepository {
  constructor(private readonly sql: SqliteRunner) {}

  // Meldet SEARCH_LOWER an der Verbindung an; muss vor der ersten Suche laufen (SqliteBoardDatabase.open)
  static register(db: DatabaseSync): void {
    db.function(SEARCH_LOWER, { deterministic: true }, (value) => (typeof value === 'string' ? value.toLowerCase() : value));
  }

  async find(request: SearchRequest): Promise<SearchRows> {
    const sources = SOURCES.filter((s) => request.kinds.includes(s.kind));
    if (!sources.length || !request.terms.length) return { rows: [], truncated: false };
    const match = new BoardSearchMatch(request.terms);
    const rows: SearchRow[] = [];
    let truncated = false;
    for (const source of sources) {
      // Ein Treffer mehr als nötig, um die Kürzung zu erkennen
      const found = await this.collect(source, request.terms, match, request.perSource + 1);
      if (found.length > request.perSource) truncated = true;
      rows.push(...found.slice(0, request.perSource));
    }
    return { rows, truncated };
  }

  // Treffer einer Quelle: alle Begriffe im selben Feld; instr statt LIKE, damit % und _ wörtlich zählen
  private async collect(source: Source, terms: string[], match: BoardSearchMatch, limit: number): Promise<SearchRow[]> {
    const params: Record<string, SqliteValue> = { take: limit };
    const conditions = source.where ? [source.where] : [];
    terms.forEach((term, i) => {
      params[`t${i}`] = term.toLowerCase();
      conditions.push(`instr(${SEARCH_LOWER}(${source.val}), @t${i}) > 0`);
    });
    const { rows } = await this.sql.query<Row>(
      `SELECT ${source.cols}, ${source.val} AS Val FROM ${source.from} WHERE ${conditions.join(' AND ')} ORDER BY ${source.order} LIMIT @take`,
      params,
    );
    return rows.filter((r) => match.matches(r.Val)).map((r) => ({
      kind: source.kind,
      id: r.Id,
      ticketId: r.TicketId,
      field: source.field,
      title: source.kind === 'note' ? r.Title.slice(0, 200) : r.Title,
      projectId: r.ProjectId,
      boardId: r.BoardId,
      status: r.Status,
      ...match.excerpt(r.Val as string),
      at: SqliteRunner.iso(r.At),
    }));
  }
}
