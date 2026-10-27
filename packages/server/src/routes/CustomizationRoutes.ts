import { OverviewFile, type HarnessHome } from '@harness/core';
import type { FastifyInstance } from 'fastify';

// GET /api/customizations: Übersicht der Anpassungen (ohne Diffs), unabhängig vom Projekt, nur lesend.
export class CustomizationRoutes {
  constructor(private readonly home: HarnessHome) {}

  // Meldet die Route an
  register(app: FastifyInstance): void {
    app.get('/api/customizations', async () => this.overview());
  }

  // Übersicht ohne Diffs; ohne Datei der Stand „unbekannt“
  private overview() {
    const overview = new OverviewFile(this.home).read();
    if (!overview) return { state: 'unknown' };
    return { ...overview, entries: overview.entries.map(({ diff: _diff, ...entry }) => entry) };
  }
}
