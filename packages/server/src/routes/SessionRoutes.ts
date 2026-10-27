import { HarnessError } from '@harness/core';
import type { FastifyInstance } from 'fastify';
import type { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import type { ProjectService } from '../projects/ProjectService.js';
import { SessionDtoMapper } from '../sessions/SessionDtoMapper.js';
import type { SessionService } from '../sessions/SessionService.js';
import type { TicketInfoLookup } from '../sessions/TicketInfoLookup.js';

// GET …/sessions: Sessions eines Projekts (letzte 7 Tage) mit Ticketangaben aus der Board-DB;
// POST …/sessions/:sid/claim: ordnet eine Session einem DB-Ticket zu und verknüpft sie dort.
export class SessionRoutes {
  private readonly mapper = new SessionDtoMapper();

  constructor(
    private readonly projects: ProjectService,
    private readonly sessions: SessionService,
    private readonly lookup: TicketInfoLookup,
    private readonly holder: BoardRuntimeHolder,
  ) {}

  // Meldet die Routen an
  register(app: FastifyInstance): void {
    app.get('/api/projects/:pid/sessions', async (request) => {
      const { pid } = request.params as { pid: string };
      await this.projects.open(pid);
      const list = this.sessions.list(pid);
      const info = await this.lookup.infos(list.flatMap((s) => s.tickets));
      return list.map((s) => this.mapper.toDto(s, info));
    });
    app.post('/api/projects/:pid/sessions/:sid/claim', async (request) => {
      const { pid, sid } = request.params as { pid: string; sid: string };
      await this.projects.open(pid);
      const ticketId = (request.body as { ticket?: unknown } | undefined)?.ticket;
      if (typeof ticketId !== 'string' || !ticketId.trim()) {
        throw new HarnessError('ticket-missing', 'Bitte das Ticket angeben (Feld ticket).');
      }
      const ticket = await this.holder.run((b) => b.services.tickets.get(ticketId));
      this.sessions.claim(pid, sid, ticket.id);
      await this.holder.run((b) => b.services.tickets.linkSession(ticket.id, sid));
      const view = this.sessions.list(pid).find((s) => s.id === sid)!;
      return this.mapper.toDto(view, new Map([[ticket.id, { id: ticket.id, title: ticket.title, status: ticket.status }]]));
    });
  }
}
