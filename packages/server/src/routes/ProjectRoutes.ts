import type { FastifyInstance } from 'fastify';
import type { ProjectService } from '../projects/ProjectService.js';

// GET/POST /api/projects: Projektliste und Anmeldung.
export class ProjectRoutes {
  constructor(private readonly projects: ProjectService) {}

  // Meldet die Routen an
  register(app: FastifyInstance): void {
    app.get('/api/projects', () => this.projects.list());
    app.post('/api/projects', async (request, reply) => {
      const body = (request.body ?? {}) as { path?: unknown };
      const { project, created } = await this.projects.register(body.path);
      return reply.code(created ? 201 : 200).send(project);
    });
  }
}
