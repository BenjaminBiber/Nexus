import { TokenStore } from '@harness/core';
import type { FastifyInstance } from 'fastify';
import type { ProjectService } from '../projects/ProjectService.js';

// GET …/settings: Projektangaben und Token für die Hooks.
export class SettingsRoutes {
  constructor(private readonly projects: ProjectService) {}

  // Meldet die Route an
  register(app: FastifyInstance): void {
    app.get('/api/projects/:pid/settings', async (request) => {
      const { project, entry } = await this.projects.open((request.params as { pid: string }).pid);
      return { project: entry, token: await new TokenStore(project.layout).read() };
    });
  }
}
