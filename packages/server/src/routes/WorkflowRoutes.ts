import { WorkflowSource, type HarnessHome } from '@harness/core';
import type { FastifyInstance } from 'fastify';
// GET /api/workflow: angewendeter Workflow (eigener oder Standard), für die Anleitung im Board.
export class WorkflowRoutes {
  constructor(private readonly home: HarnessHome) {}
  // Meldet die Route an
  register(app: FastifyInstance): void {
    app.get('/api/workflow', async () => new WorkflowSource(this.home).read());
  }
}
