import {
  SEARCH_WINDOW, type BoardSearchRepository, type SearchField, type SearchKind, type SearchRequest, type SearchRow, type SearchRows,
} from '@harness/core';
import { MssqlFeatureGate } from './MssqlFeatureGate.js';
import { SqlRunner, type SqlValue } from './SqlRunner.js';

// Groß/Klein egal, Akzente zählen (wie die Suche im Speicher); CHARINDEX braucht kein Escaping von %, _ und [
const COLLATE = 'COLLATE Latin1_General_100_CI_AS';

interface Source {
  kind: SearchKind;
  field: SearchField;
  // FROM-Teil mit Alias; liefert die Spalten für Kind/Id/… und den durchsuchten Wert
  from: string;
  value: string;
  id: string;
  ticketId: string;
  title: string;
  projectId: string;
  boardId: string;
  status: string;
  at: string;
  order: string;
}

const TICKET = {
  kind: 'ticket', from: 'board.Tickets t', id: 't.Id', ticketId: 't.Id', title: 't.Title', projectId: 't.ProjectId', boardId: 't.BoardId',
  status: 't.Status', at: 'CAST(NULL AS datetime2(3))', order: 't.CreatedAt DESC, t.Id DESC',
} as const;
const QUESTION = {
  kind: 'question', from: 'board.Questions q', id: 'q.Id', ticketId: 'CAST(NULL AS nvarchar(20))', title: 'q.Title', projectId: 'q.ProjectId',
  boardId: 'CAST(NULL AS int)', status: 'q.State', at: 'CAST(NULL AS datetime2(3))', order: 'q.CreatedAt DESC, q.Id DESC',
} as const;

// Quellen in fester Reihenfolge; je Quelle eine Teilabfrage mit TOP
const SOURCES: Source[] = [
  { ...TICKET, field: 'title', value: 't.Title' },
  { ...TICKET, field: 'description', value: 't.Description' },
  { ...TICKET, field: 'acceptance', value: 't.Acceptance' },
  { ...TICKET, field: 'plan', value: 't.[Plan]' },
  { ...TICKET, field: 'handbook', value: 't.Handbook' },
  {
    ...TICKET, field: 'comment', from: 'board.Comments c JOIN board.Tickets t ON t.Id = c.TicketId', value: 'c.Body',
    at: 'c.CreatedAt', order: 'c.Id DESC',
  },
  { ...QUESTION, field: 'title', value: 'q.Title' },
  { ...QUESTION, field: 'description', value: 'q.Description' },
  {
    ...QUESTION, field: 'decision', value: "CASE WHEN q.DecisionOutcome IS NULL THEN NULL ELSE CONCAT(q.DecisionText, NCHAR(10), q.DecisionReason) END",
    at: 'q.DecidedAt',
  },
  {
    ...QUESTION, field: 'question-comment', from: 'board.QuestionComments qc JOIN board.Questions q ON q.Id = qc.QuestionId', value: 'qc.Body',
    at: 'qc.CreatedAt', order: 'qc.Id DESC',
  },
  {
    kind: 'note', field: 'note', from: 'board.Notes n JOIN board.Boards b ON b.Id = n.BoardId', value: 'CONCAT(n.Title, NCHAR(10), n.Body)',
    id: 'CAST(n.Id AS nvarchar(20))', ticketId: 'n.TicketId', title: "CASE WHEN n.Title = N'' THEN LEFT(n.Body, 200) ELSE n.Title END", projectId: 'b.ProjectId', boardId: 'n.BoardId',
    status: 'n.State', at: 'n.CreatedAt', order: 'n.Id DESC',
  },
];

interface Row {
  Src: number; Id: string; TicketId: string | null; Title: string; ProjectId: number; BoardId: number | null; Status: string;
  At: Date | null; Txt: string; TextStart: number; TextLength: number | string;
}

// Suche über alle Projekte in einer Abfrage: UNION ALL je Quelle mit TOP (perSource + 1, um Kürzung zu erkennen);
// alle Begriffe müssen im selben Feld stehen. Übertragen wird nur ein Fenster um die erste Fundstelle des ersten Begriffs.
export class MssqlSearchRepository implements BoardSearchRepository {
  private readonly gate: MssqlFeatureGate;

  constructor(private readonly sql: SqlRunner, missingFeatures: string[] = []) {
    this.gate = new MssqlFeatureGate(missingFeatures, '');
  }

  async find(request: SearchRequest): Promise<SearchRows> {
    // Ohne note-structure fehlen board.Notes die Spalten der Notiz-Quelle: lieber ohne Notizen suchen als gar nicht
    const sources = SOURCES.map((s, i) => ({ s, i }))
      .filter(({ s }) => request.kinds.includes(s.kind) && (s.kind !== 'note' || this.gate.has('note-structure')));
    if (!sources.length || !request.terms.length) return { rows: [], truncated: false };
    const params: Record<string, SqlValue> = { take: request.perSource + 1, win: SEARCH_WINDOW };
    request.terms.forEach((t, i) => { params[`t${i}`] = t; });
    const parts = sources.map(({ s, i }) => {
      const where = request.terms.map((_, n) => `CHARINDEX(@t${n}, ${s.value} ${COLLATE}) > 0`).join(' AND ');
      return `SELECT * FROM (SELECT TOP (@take) ${i} AS Src, ${s.id} AS Id, ${s.ticketId} AS TicketId, ${s.title} AS Title,
        ${s.projectId} AS ProjectId, ${s.boardId} AS BoardId, ${s.status} AS Status, ${s.at} AS At, ${s.value} AS Val,
        ROW_NUMBER() OVER (ORDER BY ${s.order}) AS Rn
        FROM ${s.from} WHERE ${s.value} IS NOT NULL AND ${where} ORDER BY ${s.order}) AS p${i}`;
    });
    const { rows } = await this.sql.query<Row>(
      `WITH hits AS (${parts.join('\nUNION ALL\n')})
       SELECT Src, Id, TicketId, Title, ProjectId, BoardId, Status, At,
         SUBSTRING(Val, w.Start, @win * 2 + LEN(@t0)) AS Txt, w.Start - 1 AS TextStart, DATALENGTH(Val) / 2 AS TextLength
       FROM hits CROSS APPLY (SELECT CASE WHEN CHARINDEX(@t0, Val ${COLLATE}) > @win THEN CHARINDEX(@t0, Val ${COLLATE}) - @win ELSE 1 END AS Start) w
       ORDER BY Src, Rn`,
      params,
    );
    const counts = new Map<number, number>();
    const result: SearchRow[] = [];
    let truncated = false;
    for (const r of rows) {
      const seen = (counts.get(r.Src) ?? 0) + 1;
      counts.set(r.Src, seen);
      if (seen > request.perSource) {
        truncated = true;
        continue;
      }
      const s = SOURCES[r.Src];
      result.push({
        kind: s.kind, id: r.Id, ticketId: r.TicketId, field: s.field, title: r.Title, projectId: r.ProjectId, boardId: r.BoardId,
        status: r.Status, text: r.Txt, textStart: r.TextStart, textLength: Number(r.TextLength), at: SqlRunner.iso(r.At),
      });
    }
    return { rows: result, truncated };
  }
}
