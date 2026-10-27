import type { UserSettingsStore } from '@harness/core';
import type { FastifyInstance } from 'fastify';
import type { ClaudeUsageDto, ClaudeUsageService } from '../usage/ClaudeUsageService.js';

// GET /api/claude-usage: Auslastung der Claude-Abo-Limits (5-Stunden-Fenster und Woche), wie sie `/usage` zeigt. Nur wenn die
// Einstellung „claudeUsage“ eingeschaltet ist; sonst bleibt es bei `disabled`, ohne Anmeldedatei zu lesen oder Anthropic zu fragen.
export class UsageRoutes {
  constructor(
    private readonly usage: Pick<ClaudeUsageService, 'current'>,
    private readonly settings: Pick<UserSettingsStore, 'read'>,
  ) {}

  // Meldet die Route an
  register(app: FastifyInstance): void {
    app.get('/api/claude-usage', async (): Promise<ClaudeUsageDto> => {
      if (!(await this.settings.read()).claudeUsage) {
        return { available: false, reason: 'disabled', plan: null, fiveHour: null, sevenDay: null, fetchedAt: null };
      }
      return this.usage.current();
    });
  }
}
