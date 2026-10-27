import { HarnessError, SEARCH_KINDS, type SearchKind } from '@harness/core';
import type { FastifyInstance } from 'fastify';
import type { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';

// /api/db/search: Suche über alle Projekte und Boards der Board-Datenbank (nur lesend).
// Abfrage: q (2–100 Zeichen), kinds (kommagetrennt: ticket, question, note), limit (1–100).
export class DbSearchRoutes {
  constructor(private readonly holder: BoardRuntimeHolder) {}

  register(app: FastifyInstance): void {
    app.get('/api/db/search', async (request) => {
      const query = request.query as Record<string, unknown>;
      const q = typeof query.q === 'string' ? query.q : '';
      const kinds = this.kinds(query.kinds);
      const limit = this.limit(query.limit);
      return this.holder.run((b) => b.services.search.search({ q, kinds, limit }));
    });
  }

  private kinds(raw: unknown): SearchKind[] | undefined {
    if (typeof raw !== 'string' || !raw.trim()) return undefined;
    const kinds = raw.split(',').map((k) => k.trim()).filter(Boolean);
    const unknown = kinds.find((k) => !(SEARCH_KINDS as readonly string[]).includes(k));
    if (unknown) throw new HarnessError('input-invalid', `„${unknown}“ ist keine Art (ticket, question, note).`);
    return kinds as SearchKind[];
  }

  private limit(raw: unknown): number | undefined {
    if (raw === undefined || raw === '') return undefined;
    const n = Number(raw);
    if (!Number.isInteger(n) || n < 1) throw new HarnessError('input-invalid', '„limit“ muss eine ganze Zahl ab 1 sein.');
    return n;
  }
}
