import type { BoardUser, BoardUserRepository } from '@harness/core';
import { SqliteRunner } from './SqliteRunner.js';

// Benutzer in Users.
export class SqliteUserRepository implements BoardUserRepository {
  constructor(private readonly sql: SqliteRunner) {}

  // Legt an oder aktualisiert den Namen
  async upsert(user: BoardUser): Promise<void> {
    await this.sql.query('INSERT INTO Users (Id, Name) VALUES (@id, @name) ON CONFLICT(Id) DO UPDATE SET Name = excluded.Name', {
      id: user.id.toLowerCase(),
      name: user.name,
    });
  }

  async byId(id: string): Promise<BoardUser | null> {
    const { rows } = await this.sql.query<{ Id: string; Name: string }>('SELECT Id, Name FROM Users WHERE Id = @id', {
      id: id.toLowerCase(),
    });
    return rows[0] ? { id: SqliteRunner.guid(rows[0].Id), name: rows[0].Name } : null;
  }

  async list(): Promise<BoardUser[]> {
    const { rows } = await this.sql.query<{ Id: string; Name: string }>('SELECT Id, Name FROM Users ORDER BY Name COLLATE NOCASE, Id');
    return rows.map((r) => ({ id: SqliteRunner.guid(r.Id), name: r.Name }));
  }
}
