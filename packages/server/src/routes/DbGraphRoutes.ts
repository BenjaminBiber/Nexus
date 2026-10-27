import { HarnessError, type DependencyGraphView, type GraphRequest, type SpecProgressCount } from '@harness/core';
import type { FastifyInstance } from 'fastify';
import type { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import { DbInput } from '../dbboard/DbInput.js';

interface GraphQuery {
  ticket?: string;
  board?: string;
  project?: string;
  spec?: string;
  done?: string;
}

// /api/db/graph und /api/db/boards/:id/spec-progress: Abhängigkeitsgraph und Fortschritt je Spec (nur lesend).
export class DbGraphRoutes {
  private readonly input = new DbInput();

  constructor(private readonly holder: BoardRuntimeHolder) {}

  // Meldet die Routen an
  register(app: FastifyInstance): void {
    app.get('/api/db/graph', async (request): Promise<DependencyGraphView> => {
      const q = request.query as GraphQuery;
      const scope = this.scope(q);
      const done = q.done === undefined ? undefined : q.done === '1' || q.done === 'true';
      return this.holder.run((b) => b.services.graph.build(scope, done));
    });

    app.get('/api/db/boards/:id/spec-progress', async (request): Promise<SpecProgressCount[]> => {
      const id = this.input.id((request.params as { id: string }).id, 'Board');
      return this.holder.run((b) => b.services.graph.forBoard(id));
    });
  }

  // Genau ein Umfang: ticket, board oder project mit spec
  private scope(q: GraphQuery): GraphRequest {
    const ticket = q.ticket?.trim();
    const board = q.board?.trim();
    const project = q.project?.trim();
    const spec = q.spec?.trim();
    const given = [ticket, board, project || spec].filter(Boolean).length;
    if (given !== 1) throw new HarnessError('input-invalid', 'Genau einen Umfang angeben: „ticket“, „board“ oder „project“ mit „spec“.');
    if (ticket) return { kind: 'ticket', id: ticket };
    if (board) return { kind: 'board', boardId: this.input.id(board, 'Board') };
    if (!project || !spec) throw new HarnessError('input-invalid', 'Für eine Spec „project“ und „spec“ angeben.');
    return { kind: 'spec', project, spec };
  }
}
