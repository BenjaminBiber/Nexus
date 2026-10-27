import { HarnessError, type NoteState } from '@harness/core';
import type { FastifyInstance } from 'fastify';
import type { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import { DbInput } from '../dbboard/DbInput.js';

// Filter der Notizliste und die Zustände dahinter
const FILTERS: Record<string, NoteState[]> = {
  open: ['open'],
  taken: ['taken'],
  dropped: ['dropped'],
  all: ['open', 'taken', 'dropped'],
};

// /api/db/…/notes: Notizen eines Boards auflisten, anlegen, erneut gesehen zählen, als Ticket übernehmen und mit Grund verwerfen.
export class DbNoteRoutes {
  private readonly input = new DbInput();

  constructor(private readonly holder: BoardRuntimeHolder) {}

  // Meldet die Routen an
  register(app: FastifyInstance): void {
    app.get('/api/db/boards/:id/notes', async (request) => {
      const boardId = this.input.id((request.params as { id: string }).id, 'Board');
      const { state = 'open' } = request.query as { state?: string };
      if (!Object.hasOwn(FILTERS, state)) {
        throw new HarnessError('input-invalid', `Filter „${state}“ ist ungültig. Erlaubt: open, taken, dropped, all.`);
      }
      return this.holder.run((b) => b.services.notes.list(boardId, { states: FILTERS[state] }));
    });
    app.post('/api/db/boards/:id/notes', async (request, reply) => {
      const boardId = this.input.id((request.params as { id: string }).id, 'Board');
      const body = this.input.body(request.body);
      const title = this.input.text(body, 'title');
      const details = this.input.optionalText(body, 'body');
      const source = this.input.optionalText(body, 'source');
      const labels = this.input.optionalList(body, 'labels');
      const force = this.input.optionalBool(body, 'force');
      const note = await this.holder.run((b) => b.services.notes.add(boardId, { title, body: details }, b.actor(), { source, labels, force }));
      return reply.code(201).send(note);
    });
    app.post('/api/db/notes/:id/seen', async (request) => {
      const id = this.input.id((request.params as { id: string }).id, 'Notiz');
      const source = this.input.optionalText(this.input.body(request.body), 'source');
      return this.holder.run((b) => b.services.notes.seen(id, { source }));
    });
    app.post('/api/db/notes/:id/take', async (request) => {
      const id = this.input.id((request.params as { id: string }).id, 'Notiz');
      const body = this.input.body(request.body);
      const options = {
        title: this.input.optionalText(body, 'title'),
        status: this.input.optionalStatus(body, 'status'),
        priority: this.input.optionalPriority(body, 'priority'),
        labels: this.input.optionalList(body, 'labels'),
      };
      return this.holder.run((b) => b.services.notes.take(id, b.actor(), options));
    });
    app.post('/api/db/notes/:id/drop', async (request) => {
      const id = this.input.id((request.params as { id: string }).id, 'Notiz');
      const reason = this.input.text(this.input.body(request.body), 'reason');
      return this.holder.run((b) => b.services.notes.drop(id, reason));
    });
  }
}
