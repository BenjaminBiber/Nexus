import { timingSafeEqual } from 'node:crypto';
import { isAbsolute } from 'node:path';
import { HarnessError, HOOK_KINDS, TokenStore, type DisplayState, type HarnessProject, type HookEvent, type HookKind } from '@harness/core';
import type { FastifyInstance } from 'fastify';
import type { ProjectService } from '../projects/ProjectService.js';
import type { FavoriteService } from '../favorites/FavoriteService.js';
import type { SessionService } from '../sessions/SessionService.js';
import type { NotificationService } from '../notifications/NotificationService.js';
import type { TicketInfoLookup } from '../sessions/TicketInfoLookup.js';

// POST /api/hook: nimmt normalisierte Hook-Ereignisse der Sessions entgegen (mit Token des Projekts).
export class HookRoutes {
  constructor(
    private readonly projects: ProjectService,
    private readonly sessions: SessionService,
    private readonly favorites: FavoriteService,
    private readonly notifications: NotificationService,
    private readonly lookup: TicketInfoLookup,
  ) {}

  // Meldet die Route an
  register(app: FastifyInstance): void {
    app.post('/api/hook', async (request, reply) => {
      const body = (request.body ?? {}) as { project?: unknown; event?: unknown };
      if (typeof body.project !== 'string' || !isAbsolute(body.project)) {
        throw new HarnessError('path-missing', 'Der Pfad des Projekts fehlt oder ist nicht absolut.');
      }
      const { project, entry } = await this.projects.openByPath(body.project);
      await this.assertToken(project, request.headers['x-harness-token']);
      const event = this.parseEvent(body.event);
      const before = this.sessions.list(entry.id).find((s) => s.id === event.sessionId)?.displayState;
      this.sessions.apply(entry.id, event);
      await this.followFavorites(entry.id, event);
      await this.notify(entry, event, before);
      return reply.code(204).send();
    });
  }

  // Führt Favoriten nach: Momentaufnahme und Zuordnung fortgesetzter Sessions (Fehler stören den Hook nicht)
  private async followFavorites(projectId: string, event: HookEvent): Promise<void> {
    const view = this.sessions.list(projectId).find((s) => s.id === event.sessionId);
    if (!view) return;
    try {
      await this.favorites.onSession(projectId, view, event.kind === 'session-start' && event.source === 'resume');
    } catch {
      /* im Protokoll des Dienstes vermerkt */
    }
  }

  // Meldet einen Zustandswechsel an die Benachrichtigung (Fehler stören den Hook nicht)
  private async notify(entry: { id: string; name: string }, event: HookEvent, before: DisplayState | undefined): Promise<void> {
    try {
      const view = this.sessions.list(entry.id).find((s) => s.id === event.sessionId);
      if (!view || view.displayState === before) return;
      const ticket = await this.firstTicket(view.tickets[0]);
      await this.notifications.onSessionChange({ projectId: entry.id, projectName: entry.name, before, view, ticket });
    } catch {
      /* Meldung ist optional */
    }
  }

  // ID und Titel des ersten Tickets der Session aus der Board-DB, falls vorhanden
  private async firstTicket(id: string | undefined): Promise<{ id: string; title: string } | undefined> {
    if (!id) return undefined;
    const info = (await this.lookup.infos([id])).get(id);
    return { id, title: info?.title ?? '' };
  }

  // Vergleicht das Token zeitkonstant mit dem des Projekts
  private async assertToken(project: HarnessProject, given: unknown): Promise<void> {
    const expected = Buffer.from(await new TokenStore(project.layout).read());
    const actual = Buffer.from(typeof given === 'string' ? given : '');
    if (actual.length !== expected.length || !timingSafeEqual(actual, expected)) {
      throw new HarnessError('token-invalid', 'Das Token des Projekts fehlt oder ist falsch.');
    }
  }

  // Prüft das Ereignis und übernimmt nur bekannte Felder
  private parseEvent(value: unknown): HookEvent {
    const v = (value ?? {}) as Record<string, unknown>;
    const text = (x: unknown) => (typeof x === 'string' && x.trim() ? x : undefined);
    const kind = text(v.kind) as HookKind | undefined;
    const sessionId = text(v.sessionId);
    if (typeof value !== 'object' || value === null || !kind || !HOOK_KINDS.includes(kind) || !sessionId) {
      throw new HarnessError('event-invalid', 'Das Ereignis ist ungültig (kind und sessionId sind Pflicht).');
    }
    const event: HookEvent = { kind, sessionId, cwd: text(v.cwd) ?? '' };
    if (Array.isArray(v.tickets)) event.tickets = v.tickets.filter((t): t is string => typeof t === 'string' && /^[A-Za-z][A-Za-z0-9]{0,5}-\d+$/.test(t));
    for (const key of ['at', 'agentId', 'agentType', 'notificationType', 'title', 'model', 'source'] as const) {
      const found = text(v[key]);
      if (found) event[key] = found;
    }
    return event;
  }
}
