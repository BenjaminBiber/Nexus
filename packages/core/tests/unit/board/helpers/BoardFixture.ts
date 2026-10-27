import { BoardService } from '../../../../src/board/BoardService.js';
import { BoardRepoPaths, MemoryBoardRepoStore } from '../../../../src/board/BoardRepoPaths.js';
import { InMemoryBoardDatabase } from '../../../../src/board/InMemoryBoardDatabase.js';
import { CommentService } from '../../../../src/board/CommentService.js';
import { NoteService } from '../../../../src/board/NoteService.js';
import { PlanningService } from '../../../../src/board/PlanningService.js';
import { ProjectService } from '../../../../src/board/ProjectService.js';
import { TicketService } from '../../../../src/board/TicketService.js';
import { BoardQuestionService } from '../../../../src/board/BoardQuestionService.js';
import type { BoardQuestionActor } from '../../../../src/board/BoardQuestionTypes.js';
import type { BoardActor } from '../../../../src/board/BoardTypes.js';
import { USER } from './BoardDatabaseContract.js';

// Testumgebung: In-Memory-DB mit Benutzer, Projekt ARC und einem Board; die Uhr lässt sich vorstellen.
export class BoardFixture {
  now = new Date('2026-10-13T10:00:00.000Z');
  readonly clock = () => this.now;
  readonly db = new InMemoryBoardDatabase();
  readonly actor: BoardActor = { userId: USER.id, name: 'claude' };
  // Lokale Repo-Pfade des Testbenutzers, gemeinsam für alle Dienste der Fixture
  readonly repoStore = new MemoryBoardRepoStore();
  readonly repoPaths = new BoardRepoPaths(this.repoStore, USER.id);
  readonly projects = new ProjectService(this.db, this.clock);
  readonly boards = new BoardService(this.db, this.clock, this.repoPaths);
  readonly tickets = new TicketService(this.db, this.clock);
  readonly planning = new PlanningService(this.db, this.clock);
  readonly comments = new CommentService(this.db, this.clock);
  readonly notes = new NoteService(this.db, this.clock);
  readonly questions = new BoardQuestionService(this.db, this.clock, this.repoPaths);
  // Session-Agent und Mensch als Handelnde bei Fragen
  readonly agent: BoardQuestionActor = { userId: USER.id, name: 'claude', kind: 'session', session: 's-1' };
  readonly human: BoardQuestionActor = { userId: USER.id, name: 'Florian', kind: 'human', session: null };
  boardId = 0;

  // Legt Benutzer, Projekt und Board an
  static async create(): Promise<BoardFixture> {
    const f = new BoardFixture();
    await f.db.transaction((u) => u.users.upsert(USER));
    await f.projects.create('ARC', 'Arches', f.actor);
    f.boardId = (await f.boards.create('ARC', 'Sprint', f.actor)).id;
    return f;
  }

  // Stellt die Uhr um Minuten vor
  advance(minutes: number): void {
    this.now = new Date(this.now.getTime() + minutes * 60_000);
  }
}
