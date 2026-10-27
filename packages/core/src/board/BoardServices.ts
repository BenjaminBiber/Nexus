import type { BoardDatabase } from './BoardDatabase.js';
import { BoardQuestionService } from './BoardQuestionService.js';
import { BoardRepoPaths, MemoryBoardRepoStore } from './BoardRepoPaths.js';
import { BoardSearchService } from './BoardSearchService.js';
import { BoardService } from './BoardService.js';
import { BoardViewService } from './BoardViewService.js';
import { CommentService } from './CommentService.js';
import { DependencyGraphService } from './DependencyGraphService.js';
import { DispatchRunService } from './DispatchRunService.js';
import { InboxService } from './inbox/InboxService.js';
import { NoteService } from './NoteService.js';
import { PlanningService } from './PlanningService.js';
import { ProjectService } from './ProjectService.js';
import { TicketService } from './TicketService.js';
import type { TimelineAgentNames } from './TicketTimeline.js';
import { TicketTimelineService } from './TicketTimelineService.js';

// Alle Dienste des Boards über einer Datenbank (für CLI und Server). `repoPaths` liefert die eigenen Repo-Pfade des Benutzers
// (Standard: im Speicher ohne Benutzer, für Tests; die Laufzeit gibt board-repos.yaml und die eigene GUID mit).
export class BoardServices {
  readonly projects: ProjectService;
  readonly boards: BoardService;
  readonly tickets: TicketService;
  readonly planning: PlanningService;
  readonly comments: CommentService;
  readonly notes: NoteService;
  readonly views: BoardViewService;
  readonly questions: BoardQuestionService;
  readonly search: BoardSearchService;
  readonly dispatchRuns: DispatchRunService;
  readonly graph: DependencyGraphService;
  readonly inbox: InboxService;

  constructor(
    readonly db: BoardDatabase,
    clock: () => Date = () => new Date(),
    readonly repoPaths: BoardRepoPaths = new BoardRepoPaths(new MemoryBoardRepoStore(), null),
  ) {
    this.projects = new ProjectService(db, clock);
    this.boards = new BoardService(db, clock, repoPaths);
    this.tickets = new TicketService(db, clock);
    this.planning = new PlanningService(db, clock);
    this.comments = new CommentService(db, clock);
    this.notes = new NoteService(db, clock);
    this.views = new BoardViewService(db, clock);
    this.questions = new BoardQuestionService(db, clock, repoPaths);
    this.dispatchRuns = new DispatchRunService(db);
    this.search = new BoardSearchService(db);
    this.graph = new DependencyGraphService(db, clock);
    this.inbox = new InboxService(db);
  }

  // Zeitleiste eines Tickets; names aus der Dispatch-Konfiguration erkennt Agenten (null = nur „claude“ und Art des Autors)
  timeline(names: TimelineAgentNames | null): TicketTimelineService {
    return new TicketTimelineService(this.db, names);
  }
}
