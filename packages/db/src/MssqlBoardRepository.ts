import type { Board, BoardFilter, BoardRepository } from '@harness/core';
import { SqlRunner, type SqlValue } from './SqlRunner.js';

interface Row { Id: number; ProjectId: number; OwnerId: string; Name: string; RepoPath: string | null; CreatedAt: Date }

const COLUMNS = 'Id, ProjectId, OwnerId, Name, RepoPath, CreatedAt';

// Boards in board.Boards. Löschen entfernt die Notizen per Kaskade; Tickets löscht vorher der Dienst.
export class MssqlBoardRepository implements BoardRepository {
  constructor(private readonly sql: SqlRunner) {}

  async list(filter: BoardFilter = {}): Promise<Board[]> {
    const where: string[] = [];
    const params: Record<string, SqlValue> = {};
    if (filter.projectId !== undefined) { where.push('ProjectId = @projectId'); params.projectId = filter.projectId; }
    if (filter.ownerId !== undefined) { where.push('OwnerId = @ownerId'); params.ownerId = filter.ownerId; }
    const clause = where.length ? `WHERE ${where.join(' AND ')}` : '';
    return (await this.sql.query<Row>(`SELECT ${COLUMNS} FROM board.Boards ${clause} ORDER BY Id`, params)).rows.map((r) => this.map(r));
  }

  async byId(id: number): Promise<Board | null> {
    const { rows } = await this.sql.query<Row>(`SELECT ${COLUMNS} FROM board.Boards WHERE Id = @id`, { id });
    return rows[0] ? this.map(rows[0]) : null;
  }

  async insert(board: Omit<Board, 'id'>): Promise<Board> {
    const { rows } = await this.sql.query<{ Id: number }>(
      'INSERT INTO board.Boards (ProjectId, OwnerId, Name, RepoPath, CreatedAt) OUTPUT inserted.Id VALUES (@projectId, @ownerId, @name, @repoPath, @createdAt)',
      { projectId: board.projectId, ownerId: board.ownerId, name: board.name, repoPath: board.repoPath, createdAt: SqlRunner.date(board.createdAt) },
    );
    return { ...board, id: rows[0].Id };
  }

  async update(board: Board): Promise<void> {
    await this.sql.query('UPDATE board.Boards SET Name = @name, RepoPath = @repoPath WHERE Id = @id', { id: board.id, name: board.name, repoPath: board.repoPath });
  }

  async delete(id: number): Promise<void> {
    await this.sql.query('DELETE FROM board.Boards WHERE Id = @id', { id });
  }

  private map(r: Row): Board {
    return { id: r.Id, projectId: r.ProjectId, ownerId: SqlRunner.guid(r.OwnerId), name: r.Name, repoPath: r.RepoPath, createdAt: SqlRunner.iso(r.CreatedAt)! };
  }
}
