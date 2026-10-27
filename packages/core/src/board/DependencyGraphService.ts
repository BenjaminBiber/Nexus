import { HarnessError } from '../errors/HarnessError.js';
import type { BoardDatabase } from './BoardDatabase.js';
import { BoardService } from './BoardService.js';
import { DependencyGraph, type DependencyGraphResult, type GraphNode } from './DependencyGraph.js';
import { ProjectService } from './ProjectService.js';
import { SpecProgress, type SpecProgressCount } from './SpecProgress.js';
import { TicketIds } from './TicketIds.js';

// Gewünschter Umfang: ein Ticket, eine Spec eines Projekts (Key) oder ein Board
export type GraphRequest =
  | { kind: 'ticket'; id: string }
  | { kind: 'spec'; project: string; spec: string }
  | { kind: 'board'; boardId: number };

export interface GraphNodeView extends GraphNode {
  boardName: string;
}

export interface DependencyGraphView extends Omit<DependencyGraphResult, 'nodes'> {
  scope: GraphRequest;
  nodes: GraphNodeView[];
}

// Liest Tickets aller Boards und liefert Abhängigkeitsgraphen und Fortschritt je Spec (nur lesend).
export class DependencyGraphService {
  private readonly graph = new DependencyGraph();
  private readonly progress = new SpecProgress();
  private readonly ids = new TicketIds();
  private readonly projects: ProjectService;
  private readonly boards: BoardService;

  constructor(private readonly db: BoardDatabase, clock: () => Date) {
    this.projects = new ProjectService(db, clock);
    this.boards = new BoardService(db, clock);
  }

  // Graph des Umfangs; includeDone ohne Angabe: Board aus, Ticket und Spec an
  build(request: GraphRequest, includeDone?: boolean): Promise<DependencyGraphView> {
    return this.db.transaction(async (u) => {
      const tickets = await u.tickets.list({ withoutTexts: true });
      const boardNames = new Map((await u.boards.list()).map((b) => [b.id, b.name]));
      const done = includeDone ?? request.kind !== 'board';
      let scope: GraphRequest;
      let result: DependencyGraphResult;
      if (request.kind === 'ticket') {
        const id = this.ids.canonical(request.id);
        if (!tickets.some((t) => t.id === id)) throw new HarnessError('ticket-not-found', `Ticket ${id} gibt es nicht.`);
        scope = { kind: 'ticket', id };
        result = this.graph.build(tickets, { kind: 'ticket', id }, { includeDone: done });
      } else if (request.kind === 'spec') {
        const project = await this.projects.get(u, request.project);
        scope = { kind: 'spec', project: project.key, spec: request.spec };
        result = this.graph.build(tickets, { kind: 'spec', projectId: project.id, spec: request.spec }, { includeDone: done });
      } else {
        await this.boards.get(u, request.boardId);
        scope = request;
        result = this.graph.build(tickets, request, { includeDone: done });
      }
      return { ...result, scope, nodes: result.nodes.map((n) => ({ ...n, boardName: boardNames.get(n.boardId) ?? '?' })) };
    });
  }

  // Fortschritt der Specs, die auf dem Board vorkommen; gezählt wird projektweit (eine Spec gilt im ganzen Projekt)
  forBoard(boardId: number): Promise<SpecProgressCount[]> {
    return this.db.transaction(async (u) => {
      const board = await this.boards.get(u, boardId);
      const all = await u.tickets.list({ withoutTexts: true });
      const specs = new Set(all.filter((t) => t.boardId === boardId && t.spec).map((t) => t.spec));
      return this.progress.count(all.filter((t) => t.projectId === board.projectId && specs.has(t.spec)), all);
    });
  }

  // Fortschritt aller Specs eines Projekts (ID)
  forProject(projectId: number): Promise<SpecProgressCount[]> {
    return this.db.transaction(async (u) => {
      const all = await u.tickets.list({ withoutTexts: true });
      return this.progress.count(all.filter((t) => t.projectId === projectId), all);
    });
  }
}
