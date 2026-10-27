import { BoardConfigStore, HarnessError, type HarnessHome, type SpecProgressCount } from '@harness/core';
import type { FastifyInstance } from 'fastify';
import type { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import type { ProjectService } from '../projects/ProjectService.js';

// GET …/spec-progress: Stand der Specs aus dem Board.
export class SpecRoutes {
  constructor(
    private readonly projects: ProjectService,
    private readonly holder: BoardRuntimeHolder | null = null,
    private readonly home: HarnessHome | null = null,
  ) {}

  // Meldet die Routen an
  register(app: FastifyInstance): void {
    app.get('/api/projects/:pid/spec-progress', async (request): Promise<SpecProgressCount[]> => {
      const { pid } = request.params as { pid: string };
      const { project } = await this.projects.open(pid);
      return this.boardProgress(project.root);
    });
  }

  // Board des Repos (eigener Repo-Pfad) → dessen DB-Projekt → Zählung; ohne Board-Konfig, Board oder Verbindung leer
  private async boardProgress(root: string): Promise<SpecProgressCount[]> {
    if (!this.holder || !this.home) return [];
    const config = await new BoardConfigStore(this.home).read().catch(() => null);
    if (!config) return [];
    try {
      return await this.holder.run(async (b) => {
        const board = await b.services.boards.forRepo(root, { userId: config.userId, activeBoard: config.activeBoard });
        return board ? b.services.graph.forProject(board.projectId) : [];
      });
    } catch (error) {
      // Die Docs zeigen dann keinen Fortschritt
      if (error instanceof HarnessError) return [];
      throw error;
    }
  }
}
