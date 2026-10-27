import type { FastifyInstance } from 'fastify';

// GET /api/health: damit `harness board` den eigenen Server von fremden Programmen unterscheidet.
export class HealthRoutes {
  constructor(private readonly version: string) {}

  // Meldet die Route an
  register(app: FastifyInstance): void {
    app.get('/api/health', async () => ({ name: 'ai-harness', version: this.version, pid: process.pid }));
  }
}
