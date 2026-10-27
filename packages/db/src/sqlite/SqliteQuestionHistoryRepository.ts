import type { AuthorKind, BoardQuestionHistoryEntry, BoardQuestionHistoryRepository, BoardQuestionHistoryType } from '@harness/core';
import { SqliteRunner } from './SqliteRunner.js';

interface Row { QuestionId: string; At: string; ById: string; ByName: string; ByKind: AuthorKind | null; Type: BoardQuestionHistoryType; Text: string }

// Verlauf der Fragen in QuestionHistory (nur anhängen).
export class SqliteQuestionHistoryRepository implements BoardQuestionHistoryRepository {
  constructor(private readonly sql: SqliteRunner) {}

  async list(questionId: string): Promise<BoardQuestionHistoryEntry[]> {
    const { rows } = await this.sql.query<Row>(
      'SELECT QuestionId, At, ById, ByName, ByKind, "Type", "Text" FROM QuestionHistory WHERE QuestionId = @questionId ORDER BY Id',
      { questionId },
    );
    return rows.map((r) => ({
      questionId: r.QuestionId, at: SqliteRunner.iso(r.At)!, byId: SqliteRunner.guid(r.ById), byName: r.ByName, byKind: r.ByKind, type: r.Type, text: r.Text,
    }));
  }

  async insert(entries: BoardQuestionHistoryEntry[]): Promise<void> {
    for (const e of entries) {
      await this.sql.query(
        'INSERT INTO QuestionHistory (QuestionId, At, ById, ByName, ByKind, "Type", "Text") VALUES (@questionId, @at, @byId, @byName, @byKind, @type, @text)',
        { questionId: e.questionId, at: new Date(e.at), byId: e.byId.toLowerCase(), byName: e.byName, byKind: e.byKind, type: e.type, text: e.text },
      );
    }
  }
}
