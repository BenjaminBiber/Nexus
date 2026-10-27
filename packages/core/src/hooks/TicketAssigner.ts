import type { BoardServices } from '../board/BoardServices.js';

export interface AssignInput {
  cwd: string;
  sessionId: string;
  env: Record<string, string | undefined>;
  /** Board des Repos (RepoBoardResolver); null = kein Board zugeordnet */
  boardId: number | null;
}

// Ordnet eine Session Tickets des DB-Boards zu: HARNESS_TICKET, verknüpfte Session, sonst In-Arbeit-Tickets im Worktree.
export class TicketAssigner {
  constructor(
    private readonly services: Pick<BoardServices, 'tickets' | 'db'>,
    private readonly platform: string = process.platform,
  ) {}

  // Liefert die Ticket-IDs (sortiert, ohne Doppelungen); leer = „Nicht zugeordnet“
  async resolve(input: AssignInput): Promise<string[]> {
    const fromEnv = (input.env.HARNESS_TICKET ?? '').split(',').map((t) => t.trim()).filter(Boolean);
    if (fromEnv.length) {
      const known = (await this.services.tickets.summaries(fromEnv)).map((t) => t.id);
      if (known.length) return this.unique(known);
    }
    const linked = await this.services.tickets.ticketsOfSession(input.sessionId);
    if (linked.length) return this.unique(linked);
    if (input.boardId === null) return [];
    const boardId = input.boardId;
    const cwd = this.normalize(input.cwd);
    const tickets = await this.services.db.transaction((u) => u.tickets.list({ boardIds: [boardId] }));
    return this.unique(tickets
      .filter((t) => t.status === 'in_progress' && t.worktree && this.contains(this.normalize(t.worktree), cwd))
      .map((t) => t.id));
  }

  // Ist `path` der Worktree selbst oder liegt darin (ganze Pfadteile)
  private contains(worktree: string, path: string): boolean {
    return path === worktree || path.startsWith(`${worktree}/`);
  }

  // Schrägstriche, ohne Endstrich, unter Windows ohne Beachtung der Schreibweise
  private normalize(path: string): string {
    const cleaned = path.replace(/\\/g, '/').replace(/\/+$/, '');
    return this.platform === 'win32' ? cleaned.toLowerCase() : cleaned;
  }

  // Sortiert und entfernt Doppelungen
  private unique(ids: string[]): string[] {
    return [...new Set(ids)].sort();
  }
}
