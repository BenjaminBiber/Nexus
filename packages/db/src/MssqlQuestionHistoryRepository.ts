import type { AuthorKind, BoardQuestionHistoryEntry, BoardQuestionHistoryRepository, BoardQuestionHistoryType } from '@harness/core';
import { SqlRunner } from './SqlRunner.js';

interface Row { QuestionId: string; At: Date; ById: string; ByName: string; ByKind: AuthorKind | null; Type: BoardQuestionHistoryType; Text: string }

// Verlauf der Fragen in board.QuestionHistory (nur anhängen).
export class MssqlQuestionHistoryRepository implements BoardQuestionHistoryRepository {
  constructor(private readonly sql: SqlRunner) {}

  async list(questionId: string): Promise<BoardQuestionHistoryEntry[]> {
    const { rows } = await this.sql.query<Row>(
      'SELECT QuestionId, At, ById, ByName, ByKind, [Type], [Text] FROM board.QuestionHistory WHERE QuestionId = @questionId ORDER BY Id',
      { questionId },
    );
    return rows.map((r) => ({
      questionId: r.QuestionId, at: SqlRunner.iso(r.At)!, byId: SqlRunner.guid(r.ById), byName: r.ByName, byKind: r.ByKind, type: r.Type, text: r.Text,
    }));
  }

  async insert(entries: BoardQuestionHistoryEntry[]): Promise<void> {
    for (const e of entries) {
      await this.sql.query(
        'INSERT INTO board.QuestionHistory (QuestionId, At, ById, ByName, ByKind, [Type], [Text]) VALUES (@questionId, @at, @byId, @byName, @byKind, @type, @text)',
        { questionId: e.questionId, at: SqlRunner.date(e.at), byId: e.byId, byName: e.byName, byKind: e.byKind, type: e.type, text: e.text },
      );
    }
  }
}
