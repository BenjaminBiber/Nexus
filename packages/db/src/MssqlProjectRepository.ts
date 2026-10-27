import { HarnessError, type BoardProject, type BoardProjectRepository } from '@harness/core';
import { SqlRunner } from './SqlRunner.js';

interface Row { Id: number; Key: string; Name: string; NextTicketNumber: number; NextQuestionNumber: number; CreatedAt: Date; CreatedBy: string }

const COLUMNS = 'Id, [Key], Name, NextTicketNumber, NextQuestionNumber, CreatedAt, CreatedBy';

// Projekte in board.Projects; Ticket- und Fragenummern werden atomar per UPDATE … OUTPUT vergeben.
export class MssqlProjectRepository implements BoardProjectRepository {
  constructor(private readonly sql: SqlRunner) {}

  async list(): Promise<BoardProject[]> {
    return (await this.sql.query<Row>(`SELECT ${COLUMNS} FROM board.Projects ORDER BY [Key]`)).rows.map((r) => this.map(r));
  }

  async byId(id: number): Promise<BoardProject | null> {
    const { rows } = await this.sql.query<Row>(`SELECT ${COLUMNS} FROM board.Projects WHERE Id = @id`, { id });
    return rows[0] ? this.map(rows[0]) : null;
  }

  async byKey(key: string): Promise<BoardProject | null> {
    const { rows } = await this.sql.query<Row>(`SELECT ${COLUMNS} FROM board.Projects WHERE [Key] = @key`, { key });
    return rows[0] ? this.map(rows[0]) : null;
  }

  // Ein doppelter Schlüssel (SQL-Fehler 2627/2601) wird zu project-exists
  async insert(project: Omit<BoardProject, 'id' | 'nextTicketNumber' | 'nextQuestionNumber'>): Promise<BoardProject> {
    try {
      const { rows } = await this.sql.query<{ Id: number }>(
        'INSERT INTO board.Projects ([Key], Name, CreatedAt, CreatedBy) OUTPUT inserted.Id VALUES (@key, @name, @createdAt, @createdBy)',
        { key: project.key, name: project.name, createdAt: SqlRunner.date(project.createdAt), createdBy: project.createdBy },
      );
      return { ...project, id: rows[0].Id, nextTicketNumber: 1, nextQuestionNumber: 1 };
    } catch (error) {
      const number = (error as { number?: number }).number;
      if (number === 2627 || number === 2601) throw new HarnessError('project-exists', `Projekt ${project.key} gibt es schon.`);
      throw error;
    }
  }

  async rename(id: number, name: string): Promise<void> {
    await this.sql.query('UPDATE board.Projects SET Name = @name WHERE Id = @id', { id, name });
  }

  // Liefert die bisherige Ticketnummer und zählt in derselben Anweisung weiter
  async takeTicketNumber(id: number): Promise<number> {
    const { rows } = await this.sql.query<{ N: number }>(
      'UPDATE board.Projects SET NextTicketNumber = NextTicketNumber + 1 OUTPUT deleted.NextTicketNumber AS N WHERE Id = @id',
      { id },
    );
    return rows[0].N;
  }

  // Liefert die bisherige Fragenummer und zählt in derselben Anweisung weiter
  async takeQuestionNumber(id: number): Promise<number> {
    const { rows } = await this.sql.query<{ N: number }>(
      'UPDATE board.Projects SET NextQuestionNumber = NextQuestionNumber + 1 OUTPUT deleted.NextQuestionNumber AS N WHERE Id = @id',
      { id },
    );
    return rows[0].N;
  }

  private map(r: Row): BoardProject {
    return {
      id: r.Id, key: r.Key, name: r.Name, nextTicketNumber: r.NextTicketNumber, nextQuestionNumber: r.NextQuestionNumber,
      createdAt: SqlRunner.iso(r.CreatedAt)!, createdBy: SqlRunner.guid(r.CreatedBy),
    };
  }
}
