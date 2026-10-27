import { HarnessError } from '@harness/core';
import type { FastifyInstance } from 'fastify';
import { FavoriteDtoMapper } from '../favorites/FavoriteDtoMapper.js';
import type { FavoriteService } from '../favorites/FavoriteService.js';
import type { ProjectService } from '../projects/ProjectService.js';
import type { SessionService } from '../sessions/SessionService.js';
import type { TicketInfoLookup } from '../sessions/TicketInfoLookup.js';

// …/favorites: Favoriten eines Projekts lesen, markieren (mit Notiz) und entfernen.
export class FavoriteRoutes {
  private readonly mapper = new FavoriteDtoMapper();

  constructor(
    private readonly projects: ProjectService,
    private readonly sessions: SessionService,
    private readonly favorites: FavoriteService,
    private readonly lookup: TicketInfoLookup,
  ) {}

  // Meldet die Routen an
  register(app: FastifyInstance): void {
    app.get('/api/projects/:pid/favorites', async (request) => {
      const { pid } = request.params as { pid: string };
      return this.list(pid);
    });
    app.put('/api/projects/:pid/favorites/:sid', async (request) => {
      const { pid, sid } = request.params as { pid: string; sid: string };
      await this.projects.open(pid);
      const note = this.note((request.body ?? {}) as { note?: unknown });
      const live = this.sessions.list(pid).find((s) => s.id === sid);
      const known = this.favorites.list(pid).some((f) => f.sessionId === sid);
      if (!live && known) await this.favorites.setNote(pid, sid, note ?? '');
      else await this.favorites.mark(pid, live, note);
      return (await this.list(pid)).find((f) => f.sessionId === sid);
    });
    app.delete('/api/projects/:pid/favorites/:sid', async (request, reply) => {
      const { pid, sid } = request.params as { pid: string; sid: string };
      await this.projects.open(pid);
      await this.favorites.unmark(pid, sid);
      return reply.code(204).send();
    });
  }

  // Favoriten mit Live-Zustand und Ticketangaben aus der Board-DB
  private async list(pid: string) {
    await this.projects.open(pid);
    const live = new Map(this.sessions.list(pid).map((s) => [s.id, s]));
    const favorites = this.favorites.list(pid);
    const info = await this.lookup.infos(favorites.flatMap((f) => live.get(f.sessionId)?.tickets ?? f.tickets));
    return favorites.map((f) => this.mapper.toDto(f, live.get(f.sessionId), info));
  }

  // Notiz aus dem Body: Text oder nicht angegeben
  private note(body: { note?: unknown }): string | undefined {
    if (body.note === undefined) return undefined;
    if (typeof body.note !== 'string') throw new HarnessError('note-invalid', 'Die Notiz muss ein Text sein.');
    return body.note.slice(0, 2000);
  }
}
