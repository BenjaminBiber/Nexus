import { HarnessError } from '../errors/HarnessError.js';
import { SearchSnippet } from '../text/SearchSnippet.js';
import type { BoardDatabase } from './BoardDatabase.js';
import {
  SEARCH_DEFAULT_LIMIT, SEARCH_KINDS, SEARCH_MAX_LENGTH, SEARCH_MAX_LIMIT, SEARCH_MAX_TERMS, SEARCH_MIN_LENGTH, SEARCH_PER_SOURCE,
  type SearchField, type SearchHit, type SearchKind, type SearchResult, type SearchRow,
} from './BoardSearchTypes.js';

export interface SearchInput {
  q: string;
  kinds?: SearchKind[];
  limit?: number;
}

// Kleinere Zahl steht weiter oben: Titeltreffer zuerst, Kommentare und Notizen zuletzt
const FIELD_RANK: Record<SearchField, number> = {
  title: 0, acceptance: 1, description: 1, decision: 1, plan: 2, handbook: 2, comment: 3, 'question-comment': 3, note: 3,
};
const KIND_RANK: Record<SearchKind, number> = { ticket: 0, question: 1, note: 2 };
const NOTE_TITLE_LENGTH = 80;

// Suche über alle Projekte und Boards der Board-Datenbank: Titel, Beschreibung, Kommentare, Plan, Handbuch, Abnahme,
// Fragen (samt Entscheidung und Kommentaren) und Notizen. Je Ticket, Frage oder Notiz ein Treffer (bestes Feld).
export class BoardSearchService {
  private readonly snippets = new SearchSnippet();

  constructor(private readonly db: BoardDatabase) {}

  search(input: SearchInput): Promise<SearchResult> {
    const query = input.q.replace(/\s+/g, ' ').trim();
    if (query.length < SEARCH_MIN_LENGTH) {
      return Promise.reject(new HarnessError('query-short', `Bitte mindestens ${SEARCH_MIN_LENGTH} Zeichen eingeben.`));
    }
    if (query.length > SEARCH_MAX_LENGTH) {
      return Promise.reject(new HarnessError('query-long', `Höchstens ${SEARCH_MAX_LENGTH} Zeichen.`));
    }
    const terms = [...new Map(query.split(' ').map((t) => [t.toLowerCase(), t])).values()];
    if (terms.length > SEARCH_MAX_TERMS) {
      return Promise.reject(new HarnessError('query-long', `Höchstens ${SEARCH_MAX_TERMS} Wörter.`));
    }
    const kinds = input.kinds?.length ? SEARCH_KINDS.filter((k) => input.kinds!.includes(k)) : [...SEARCH_KINDS];
    const limit = Math.min(Math.max(Math.trunc(input.limit ?? SEARCH_DEFAULT_LIMIT), 1), SEARCH_MAX_LIMIT);
    return this.db.transaction(async (u) => {
      const found = await u.search.find({ terms, kinds, perSource: SEARCH_PER_SOURCE });
      const projects = new Map((await u.projects.list()).map((p) => [p.id, p]));
      const boards = new Map((await u.boards.list()).map((b) => [b.id, b]));
      const best = new Map<string, SearchRow>();
      for (const row of [...found.rows].sort((a, b) => this.compare(a, b))) {
        const key = `${row.kind}:${row.id}`;
        if (!best.has(key)) best.set(key, row);
      }
      const rows = [...best.values()];
      const hits = rows.slice(0, limit).map((row): SearchHit => {
        const project = projects.get(row.projectId);
        const board = row.boardId === null ? undefined : boards.get(row.boardId);
        const snippet = this.snippets.build(row.text, terms, {
          cutBefore: row.textStart > 0, cutAfter: row.textStart + row.text.length < row.textLength,
        });
        return {
          kind: row.kind, id: row.id, ticketId: row.ticketId, field: row.field,
          title: row.kind === 'note' ? this.noteTitle(row.title) : row.title,
          project: { id: row.projectId, key: project?.key ?? '', name: project?.name ?? '' },
          board: board ? { id: board.id, name: board.name } : null,
          status: row.status, snippet: snippet.text, matches: snippet.matches, at: row.at,
        };
      });
      return { query, terms, hits, truncated: found.truncated || rows.length > limit };
    });
  }

  private compare(a: SearchRow, b: SearchRow): number {
    return FIELD_RANK[a.field] - FIELD_RANK[b.field]
      || KIND_RANK[a.kind] - KIND_RANK[b.kind]
      || (b.at ?? '').localeCompare(a.at ?? '')
      || b.id.localeCompare(a.id, undefined, { numeric: true });
  }

  // Erste Zeile der Notiz, gekürzt
  private noteTitle(body: string): string {
    const line = body.trim().split(/\r?\n/)[0].trim();
    return line.length > NOTE_TITLE_LENGTH ? `${line.slice(0, NOTE_TITLE_LENGTH - 1)}…` : line;
  }
}
