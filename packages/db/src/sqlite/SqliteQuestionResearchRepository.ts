import type { AuthorKind, BoardQuestionResearch, BoardQuestionResearchRepository, BoardQuestionResearchSummary } from '@harness/core';
import { SqliteRunner, type SqliteValue } from './SqliteRunner.js';

interface ResearchRow { Id: number; QuestionId: string; At: string; ById: string; ByName: string; Kind: AuthorKind; Context: string }
interface OptionRow { ResearchId: number; OptionId: string; Title: string; Text: string; Pros: string; Cons: string; Recommended: number }

// Recherchen in QuestionResearch mit ihren Optionen in QuestionOptions (nur anhängen).
export class SqliteQuestionResearchRepository implements BoardQuestionResearchRepository {
  constructor(private readonly sql: SqliteRunner) {}

  // Recherchen in Reihenfolge des Anlegens, Optionen nach Recherche und Nummer
  async list(questionId: string): Promise<BoardQuestionResearch[]> {
    const research = await this.sql.query<ResearchRow>(
      'SELECT Id, QuestionId, At, ById, ByName, Kind, Context FROM QuestionResearch WHERE QuestionId = @questionId ORDER BY Id',
      { questionId },
    );
    if (!research.rows.length) return [];
    const options = await this.sql.query<OptionRow>(
      `SELECT ResearchId, OptionId, Title, Text, Pros, Cons, Recommended FROM QuestionOptions
       WHERE QuestionId = @questionId ORDER BY ResearchId, CAST(substr(OptionId, 2) AS INTEGER)`,
      { questionId },
    );
    return research.rows.map((r) => ({
      id: Number(r.Id), questionId: r.QuestionId, at: SqliteRunner.iso(r.At)!, byId: SqliteRunner.guid(r.ById), byName: r.ByName,
      kind: r.Kind, context: r.Context,
      options: options.rows.filter((o) => Number(o.ResearchId) === Number(r.Id)).map((o) => ({
        id: o.OptionId, title: o.Title, text: o.Text, pros: JSON.parse(o.Pros) as string[], cons: JSON.parse(o.Cons) as string[],
        recommended: SqliteRunner.bool(o.Recommended),
      })),
    }));
  }

  async insert(r: Omit<BoardQuestionResearch, 'id'>): Promise<BoardQuestionResearch> {
    const { rows } = await this.sql.query<{ Id: number }>(
      'INSERT INTO QuestionResearch (QuestionId, At, ById, ByName, Kind, Context) VALUES (@questionId, @at, @byId, @byName, @kind, @context) RETURNING Id',
      { questionId: r.questionId, at: new Date(r.at), byId: r.byId.toLowerCase(), byName: r.byName, kind: r.kind, context: r.context },
    );
    const id = Number(rows[0].Id);
    for (const o of r.options) {
      await this.sql.query(
        `INSERT INTO QuestionOptions (QuestionId, ResearchId, OptionId, Title, Text, Pros, Cons, Recommended)
         VALUES (@questionId, @researchId, @optionId, @title, @text, @pros, @cons, @recommended)`,
        { questionId: r.questionId, researchId: id, optionId: o.id, title: o.title, text: o.text, pros: JSON.stringify(o.pros), cons: JSON.stringify(o.cons), recommended: o.recommended },
      );
    }
    return { ...r, id };
  }

  // Zahl der Recherchen und Optionen der neuesten je Frage
  async summaries(questionIds: string[]): Promise<Record<string, BoardQuestionResearchSummary>> {
    if (!questionIds.length) return {};
    const params: Record<string, SqliteValue> = {};
    questionIds.forEach((id, i) => {
      params[`q${i}`] = id;
    });
    const { rows } = await this.sql.query<{ QuestionId: string; N: number; Latest: number }>(
      `SELECT QuestionId, COUNT(*) AS N, MAX(Id) AS Latest FROM QuestionResearch
       WHERE QuestionId IN (${questionIds.map((_, i) => `@q${i}`).join(', ')}) GROUP BY QuestionId`,
      params,
    );
    if (!rows.length) return {};
    const latest: Record<string, SqliteValue> = {};
    rows.forEach((r, i) => {
      latest[`r${i}`] = Number(r.Latest);
    });
    const options = await this.sql.query<{ ResearchId: number; N: number }>(
      `SELECT ResearchId, COUNT(*) AS N FROM QuestionOptions WHERE ResearchId IN (${rows.map((_, i) => `@r${i}`).join(', ')}) GROUP BY ResearchId`,
      latest,
    );
    const counts = new Map(options.rows.map((o) => [Number(o.ResearchId), Number(o.N)]));
    return Object.fromEntries(rows.map((r) => [r.QuestionId, { count: Number(r.N), optionCount: counts.get(Number(r.Latest)) ?? 0 }]));
  }
}
