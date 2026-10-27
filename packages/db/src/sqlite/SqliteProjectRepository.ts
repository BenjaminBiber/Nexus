import { HarnessError, type BoardProject, type BoardProjectRepository } from '@harness/core';
import { SqlErrorMapper } from '../SqlErrorMapper.js';
import { SqliteRunner } from './SqliteRunner.js';

interface Row { Id: number; Key: string; Name: string; NextTicketNumber: number; NextQuestionNumber: number; CreatedAt: string; CreatedBy: string }

const COLUMNS = 'Id, "Key", Name, NextTicketNumber, NextQuestionNumber, CreatedAt, CreatedBy';

// Projekte in Projects; Ticket- und Fragenummern werden atomar per UPDATE … RETURNING vergeben.
export class SqliteProjectRepository implements BoardProjectRepository {
  constructor(private readonly sql: SqliteRunner) {}

  async list(): Promise<BoardProject[]> {
    return (await this.sql.query<Row>(`SELECT ${COLUMNS} FROM Projects ORDER BY "Key"`)).rows.map((r) => this.map(r));
  }

  async byId(id: number): Promise<BoardProject | null> {
    const { rows } = await this.sql.query<Row>(`SELECT ${COLUMNS} FROM Projects WHERE Id = @id`, { id });
    return rows[0] ? this.map(rows[0]) : null;
  }

  async byKey(key: string): Promise<BoardProject | null> {
    const { rows } = await this.sql.query<Row>(`SELECT ${COLUMNS} FROM Projects WHERE "Key" = @key`, { key });
    return rows[0] ? this.map(rows[0]) : null;
  }

  // Ein doppelter Schlüssel (SQLite-Eindeutigkeitsfehler) wird zu project-exists
  async insert(project: Omit<BoardProject, 'id' | 'nextTicketNumber' | 'nextQuestionNumber'>): Promise<BoardProject> {
    try {
      const { rows } = await this.sql.query<{ Id: number }>(
        'INSERT INTO Projects ("Key", Name, CreatedAt, CreatedBy) VALUES (@key, @name, @createdAt, @createdBy) RETURNING Id',
        { key: project.key, name: project.name, createdAt: new Date(project.createdAt), createdBy: project.createdBy.toLowerCase() },
      );
      return { ...project, id: Number(rows[0].Id), nextTicketNumber: 1, nextQuestionNumber: 1 };
    } catch (error) {
      if (SqlErrorMapper.isUniqueViolation(error)) throw new HarnessError('project-exists', `Projekt ${project.key} gibt es schon.`);
      throw error;
    }
  }

  async rename(id: number, name: string): Promise<void> {
    await this.sql.query('UPDATE Projects SET Name = @name WHERE Id = @id', { id, name });
  }

  // Liefert die bisherige Ticketnummer und zählt in derselben Anweisung weiter (RETURNING liefert den neuen Wert)
  async takeTicketNumber(id: number): Promise<number> {
    const { rows } = await this.sql.query<{ N: number }>(
      'UPDATE Projects SET NextTicketNumber = NextTicketNumber + 1 WHERE Id = @id RETURNING NextTicketNumber - 1 AS N',
      { id },
    );
    return Number(rows[0].N);
  }

  // Liefert die bisherige Fragenummer und zählt in derselben Anweisung weiter (RETURNING liefert den neuen Wert)
  async takeQuestionNumber(id: number): Promise<number> {
    const { rows } = await this.sql.query<{ N: number }>(
      'UPDATE Projects SET NextQuestionNumber = NextQuestionNumber + 1 WHERE Id = @id RETURNING NextQuestionNumber - 1 AS N',
      { id },
    );
    return Number(rows[0].N);
  }

  private map(r: Row): BoardProject {
    return {
      id: Number(r.Id), key: r.Key, name: r.Name, nextTicketNumber: Number(r.NextTicketNumber), nextQuestionNumber: Number(r.NextQuestionNumber),
      createdAt: SqliteRunner.iso(r.CreatedAt)!, createdBy: SqliteRunner.guid(r.CreatedBy),
    };
  }
}
