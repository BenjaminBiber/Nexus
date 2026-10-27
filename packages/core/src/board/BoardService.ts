import { HarnessError } from '../errors/HarnessError.js';
import type { BoardDatabase, BoardUnitOfWork } from './BoardDatabase.js';
import { MAX_BOARD_NAME_LENGTH, type Board, type BoardActor } from './BoardTypes.js';
import { BoardLimits } from './BoardLimits.js';
import { DependencyCleanup } from './DependencyCleanup.js';
import { QuestionTicketCleanup } from './QuestionTicketCleanup.js';
import { ProjectService } from './ProjectService.js';
import { RepoBoardResolver, type RepoBoardOwner } from './RepoBoardResolver.js';
import { BoardRepoPaths, MemoryBoardRepoStore, type BoardRepoSource } from './BoardRepoPaths.js';

// `repoPath` ist der wirksame Pfad des Benutzers (lokal oder Altbestand), `repoSource` seine Herkunft
export interface BoardSummary extends Board {
  projectKey: string;
  projectName: string;
  ownerName: string;
  openTickets: number;
  repoSource: BoardRepoSource | null;
}

// Boards eines Projekts: anlegen, auflisten, umbenennen, eigenen Repo-Pfad setzen, löschen.
// Alle gelieferten Boards tragen in `repoPath` den wirksamen Pfad des Benutzers (BoardRepoPaths), nur `get` den DB-Wert.
export class BoardService {
  private readonly projects: ProjectService;
  private readonly repos = new RepoBoardResolver();
  private readonly cleanup = new DependencyCleanup();
  private readonly questionCleanup = new QuestionTicketCleanup();

  constructor(
    private readonly db: BoardDatabase,
    private readonly clock: () => Date,
    private readonly repoPaths: BoardRepoPaths = new BoardRepoPaths(new MemoryBoardRepoStore(), null),
  ) {
    this.projects = new ProjectService(db, clock);
  }

  // Boards mit Projekt, Besitzer und Zahl der offenen Tickets, sortiert nach Projekt und Name
  async list(filter: { projectKey?: string; ownerId?: string } = {}): Promise<BoardSummary[]> {
    const list = await this.db.transaction(async (u) => {
      const projectId = filter.projectKey ? (await this.projects.get(u, filter.projectKey)).id : undefined;
      const boards = await u.boards.list({ projectId, ownerId: filter.ownerId });
      const projects = new Map((await u.projects.list()).map((p) => [p.id, p]));
      const users = new Map((await u.users.list()).map((x) => [x.id, x.name]));
      const open = boards.length ? await u.tickets.openCountByBoard(boards.map((b) => b.id)) : {};
      return boards
        .map((b) => ({
          ...b,
          projectKey: projects.get(b.projectId)?.key ?? '?',
          projectName: projects.get(b.projectId)?.name ?? '?',
          ownerName: users.get(b.ownerId) ?? b.ownerId,
          openTickets: open[b.id] ?? 0,
        }))
        .sort((a, b) => a.projectKey.localeCompare(b.projectKey) || a.name.localeCompare(b.name));
    });
    return this.repoPaths.apply(list);
  }

  // Eigene Boards eines Benutzers mit wirksamem Repo-Pfad
  async owned(ownerId: string): Promise<Board[]> {
    return this.repoPaths.apply(await this.db.transaction((u) => u.boards.list({ ownerId })));
  }

  // Board zur ID mit wirksamem Repo-Pfad oder null
  async find(id: number): Promise<Board | null> {
    const board = await this.db.transaction((u) => u.boards.byId(id));
    return board ? this.repoPaths.one(board) : null;
  }

  // Board zur ID innerhalb einer Transaktion oder Fehler; `repoPath` ist hier der DB-Wert (Altbestand), nicht der wirksame Pfad
  async get(u: BoardUnitOfWork, id: number): Promise<Board> {
    const board = await u.boards.byId(id);
    if (!board) throw new HarnessError('board-not-found', `Board ${id} gibt es nicht.`);
    return board;
  }

  // Board zur ID mit wirksamem Repo-Pfad in eigener Transaktion oder Fehler 'board-not-found'
  async require(id: number): Promise<Board> {
    return this.repoPaths.one(await this.db.transaction((u) => this.get(u, id)));
  }

  // Board, dessen wirksamer Repo-Pfad zum Hauptordner des Git-Repos passt, oder null
  async forRepo(repoRoot: string, me: RepoBoardOwner): Promise<Board | null> {
    const boards = await this.repoPaths.apply(await this.db.transaction((u) => u.boards.list()));
    return this.repos.pick(boards, repoRoot, me);
  }

  // Wie forRepo, ohne Treffer mit Hinweis auf „harness board boards repo“
  async requireForRepo(repoRoot: string, me: RepoBoardOwner): Promise<Board> {
    const board = await this.forRepo(repoRoot, me);
    if (!board) {
      throw new HarnessError('board-repo-unknown', `Kein Board ist dem Repo ${repoRoot} zugeordnet. Mit „harness board boards repo <id> ${repoRoot}“ zuordnen.`);
    }
    return board;
  }

  // Legt ein Board im Projekt an; Besitzer ist der handelnde Benutzer
  async create(projectKey: string, name: string, actor: BoardActor): Promise<Board> {
    const title = this.requireName(name);
    const created = await this.db.transaction(async (u) => {
      const project = await this.projects.get(u, projectKey);
      return u.boards.insert({ projectId: project.id, ownerId: actor.userId, name: title, repoPath: null, createdAt: this.clock().toISOString() });
    });
    return this.repoPaths.one(created);
  }

  // Benennt ein Board um
  async rename(id: number, name: string): Promise<Board> {
    const title = this.requireName(name);
    return this.change(id, (b) => ({ ...b, name: title }));
  }

  // Setzt den eigenen Repo-Pfad des Benutzers (lokal, nicht in der gemeinsamen DB); leer oder null hinterlegt „kein Pfad“
  async setRepoPath(id: number, path: string | null): Promise<Board & { repoSource: BoardRepoSource | null }> {
    const board = await this.db.transaction((u) => this.get(u, id));
    await this.repoPaths.set(id, path);
    return this.repoPaths.one(board);
  }

  // Löscht ein Board; offene Tickets nur mit deleteOpen. Jede Ticketlöschung wird protokolliert.
  delete(id: number, options: { deleteOpen: boolean }, actor: BoardActor): Promise<{ deletedTickets: string[] }> {
    return this.db.transaction(async (u) => {
      BoardLimits.actor(actor);
      await this.get(u, id);
      const tickets = (await u.tickets.list({ boardIds: [id] })).sort((a, b) => a.number - b.number);
      const open = tickets.filter((t) => t.status !== 'done' && t.status !== 'not_needed');
      if (open.length && !options.deleteOpen) {
        throw new HarnessError('board-has-open-tickets', `Auf dem Board liegen noch offene Tickets: ${open.map((t) => t.id).join(', ')}. Erst verschieben oder ausdrücklich alles löschen.`);
      }
      const at = this.clock().toISOString();
      const deleted = new Set(tickets.map((t) => t.id));
      for (const ticket of tickets) await this.cleanup.remove(u, ticket.id, actor, at, deleted);
      for (const ticket of tickets) await this.questionCleanup.remove(u, ticket, actor, at);
      for (const ticket of tickets) {
        await u.tickets.delete(ticket.id);
        await u.deletions.insert({ at, byId: actor.userId, byName: actor.name, ticketId: ticket.id, ticketTitle: ticket.title, fromStatus: ticket.status });
      }
      await u.boards.delete(id);
      return { deletedTickets: tickets.map((t) => t.id) };
    });
  }

  // Liest, ändert und schreibt ein Board in einer Transaktion
  // (der DB-Wert des Repo-Pfads bleibt unverändert; geliefert wird das Board mit wirksamem Pfad)
  private async change(id: number, apply: (board: Board) => Board): Promise<Board> {
    const next = await this.db.transaction(async (u) => {
      const changed = apply(await this.get(u, id));
      await u.boards.update(changed);
      return changed;
    });
    return this.repoPaths.one(next);
  }

  // Name ohne Leerraum am Rand; leer oder zu lang ist nicht erlaubt
  private requireName(name: string): string {
    const title = name.trim();
    if (!title) throw new HarnessError('board-name-empty', 'Der Boardname darf nicht leer sein.');
    return BoardLimits.text(title, MAX_BOARD_NAME_LENGTH, 'Der Boardname');
  }
}
