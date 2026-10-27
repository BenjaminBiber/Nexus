import type { AuthorKind, BoardQuestionComment, BoardQuestionCommentRepository } from '@harness/core';
import { SqliteRunner } from './SqliteRunner.js';

interface Row { Id: number; QuestionId: string; AuthorId: string; AuthorName: string; AuthorKind: AuthorKind; Body: string; CreatedAt: string }

// Kommentare an Fragen in QuestionComments (nur anhängen).
export class SqliteQuestionCommentRepository implements BoardQuestionCommentRepository {
  constructor(private readonly sql: SqliteRunner) {}

  async list(questionId: string): Promise<BoardQuestionComment[]> {
    const { rows } = await this.sql.query<Row>(
      'SELECT Id, QuestionId, AuthorId, AuthorName, AuthorKind, Body, CreatedAt FROM QuestionComments WHERE QuestionId = @questionId ORDER BY Id',
      { questionId },
    );
    return rows.map((r) => ({
      id: Number(r.Id), questionId: r.QuestionId, authorId: SqliteRunner.guid(r.AuthorId), authorName: r.AuthorName, authorKind: r.AuthorKind,
      body: r.Body, createdAt: SqliteRunner.iso(r.CreatedAt)!,
    }));
  }

  async insert(c: Omit<BoardQuestionComment, 'id'>): Promise<BoardQuestionComment> {
    const { rows } = await this.sql.query<{ Id: number }>(
      `INSERT INTO QuestionComments (QuestionId, AuthorId, AuthorName, AuthorKind, Body, CreatedAt)
       VALUES (@questionId, @authorId, @authorName, @authorKind, @body, @createdAt) RETURNING Id`,
      { questionId: c.questionId, authorId: c.authorId.toLowerCase(), authorName: c.authorName, authorKind: c.authorKind, body: c.body, createdAt: new Date(c.createdAt) },
    );
    return { ...c, id: Number(rows[0].Id) };
  }
}
