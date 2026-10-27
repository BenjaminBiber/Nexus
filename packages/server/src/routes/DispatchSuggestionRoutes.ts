import type { FastifyInstance } from 'fastify';
import type { DispatchSuggestionService } from '../dispatch/DispatchSuggestionService.js';
import type { SuggestionAnalyzer } from '../dispatch/SuggestionAnalyzer.js';

// /api/dispatch/suggestions: Optimierungs-Vorschläge zur Dispatcher-Einstellung – Liste, Übernehmen, Aus- und Einblenden, Claude-Analyse.
export class DispatchSuggestionRoutes {
  constructor(private readonly service: DispatchSuggestionService, private readonly analyzer: SuggestionAnalyzer) {}

  // Meldet die Routen an
  register(app: FastifyInstance): void {
    app.get('/api/dispatch/suggestions', async () => this.service.list());
    app.post('/api/dispatch/suggestions/:id/apply', async (request) =>
      this.service.apply((request.params as { id: string }).id, (request.body as { configHash?: unknown } | undefined)?.configHash));
    app.post('/api/dispatch/suggestions/:id/dismiss', async (request) => this.service.dismiss((request.params as { id: string }).id, true));
    app.delete('/api/dispatch/suggestions/:id/dismiss', async (request) => this.service.dismiss((request.params as { id: string }).id, false));
    app.post('/api/dispatch/suggestions/analyze', async () => this.analyzer.analyze());
  }
}
