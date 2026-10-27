import type { BoardUser, BoardUserRepository } from '@harness/core';
import { SqlRunner } from './SqlRunner.js';

// Benutzer in board.Users.
export class MssqlUserRepository implements BoardUserRepository {
  constructor(private readonly sql: SqlRunner) {}

  // Legt an oder aktualisiert den Namen; UPDLOCK/SERIALIZABLE sperrt den Schlüsselbereich gegen gleichzeitiges Erst-Anlegen
  async upsert(user: BoardUser): Promise<void> {
    await this.sql.query(
      `UPDATE board.Users WITH (UPDLOCK, SERIALIZABLE) SET Name = @name WHERE Id = @id;
       IF @@ROWCOUNT = 0 INSERT INTO board.Users (Id, Name) VALUES (@id, @name);`,
      { id: user.id, name: user.name },
    );
  }

  async byId(id: string): Promise<BoardUser | null> {
    const { rows } = await this.sql.query<{ Id: string; Name: string }>('SELECT Id, Name FROM board.Users WHERE Id = @id', { id });
    return rows[0] ? { id: SqlRunner.guid(rows[0].Id), name: rows[0].Name } : null;
  }

  async list(): Promise<BoardUser[]> {
    const { rows } = await this.sql.query<{ Id: string; Name: string }>('SELECT Id, Name FROM board.Users ORDER BY Name');
    return rows.map((r) => ({ id: SqlRunner.guid(r.Id), name: r.Name }));
  }
}
