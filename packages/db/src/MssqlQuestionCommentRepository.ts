import type { AuthorKind, BoardQuestionComment, BoardQuestionCommentRepository } from '@harness/core';
import { SqlRunner } from './SqlRunner.js';

interface Row { Id: string | number; QuestionId: string; AuthorId: string; AuthorName: string; AuthorKind: AuthorKind; Body: string; CreatedAt: Date }

// Kommentare an Fragen in board.QuestionComments (nur anhängen).
export class MssqlQuestionCommentRepository implements BoardQuestionCommentRepository {
  constructor(private readonly sql: SqlRunner) {}

  async list(questionId: string): Promise<BoardQuestionComment[]> {
    const { rows } = await this.sql.query<Row>(
      'SELECT Id, QuestionId, AuthorId, AuthorName, AuthorKind, Body, CreatedAt FROM board.QuestionComments WHERE QuestionId = @questionId ORDER BY Id',
      { questionId },
    );
    return rows.map((r) => ({
      id: Number(r.Id), questionId: r.QuestionId, authorId: SqlRunner.guid(r.AuthorId), authorName: r.AuthorName, authorKind: r.AuthorKind,
      body: r.Body, createdAt: SqlRunner.iso(r.CreatedAt)!,
    }));
  }

  async insert(c: Omit<BoardQuestionComment, 'id'>): Promise<BoardQuestionComment> {
    const { rows } = await this.sql.query<{ Id: string | number }>(
      `INSERT INTO board.QuestionComments (QuestionId, AuthorId, AuthorName, AuthorKind, Body, CreatedAt)
       OUTPUT inserted.Id VALUES (@questionId, @authorId, @authorName, @authorKind, @body, @createdAt)`,
      { questionId: c.questionId, authorId: c.authorId, authorName: c.authorName, authorKind: c.authorKind, body: c.body, createdAt: SqlRunner.date(c.createdAt) },
    );
    return { ...c, id: Number(rows[0].Id) };
  }
}
