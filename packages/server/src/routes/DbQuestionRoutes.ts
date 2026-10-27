import { HarnessError, QUESTION_STATES, type BoardQuestionActor, type BoardQuestionDetail, type QuestionState } from '@harness/core';
import type { FastifyInstance, FastifyReply } from 'fastify';
import type { BoardBackend } from '../dbboard/BoardBackend.js';
import type { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import { DbInput, type Body } from '../dbboard/DbInput.js';

// /api/db/projects/:key/questions und /api/db/questions/:id…: Fragen eines Board-Projekts lesen, stellen, ändern, verknüpfen,
// kommentieren und entscheiden. Handelnder ist der Mensch der Oberfläche (GUID und Name aus board.yaml).
export class DbQuestionRoutes {
  private readonly input = new DbInput();

  constructor(private readonly holder: BoardRuntimeHolder) {}

  // Meldet die Routen an
  register(app: FastifyInstance): void {
    app.get('/api/db/projects/:key/questions', async (request) => {
      const { key } = request.params as { key: string };
      const query = request.query as Record<string, unknown>;
      const filter = {
        state: this.state(this.param(query, 'state')),
        ticket: this.param(query, 'ticket') || undefined,
        tag: this.param(query, 'tag') || undefined,
        text: this.param(query, 'q') || undefined,
      };
      // Betrachter ist der Benutzer der Oberfläche: `mine` und counts.mine („Eigene“) rechnet der Server
      return this.holder.run((b) => b.services.questions.list(key, filter, b.actor().userId));
    });
    app.post('/api/db/projects/:key/questions', async (request, reply) => {
      const { key } = request.params as { key: string };
      const body = this.input.body(request.body);
      const input = {
        title: this.input.text(body, 'title'),
        description: this.input.optionalText(body, 'description'),
        tags: this.input.optionalList(body, 'tags'),
        occasion: this.input.nullableText(body, 'occasion'),
        ticketIds: this.input.optionalList(body, 'ticketIds'),
        dependsOn: this.input.optionalList(body, 'dependsOn'),
      };
      const created = await this.holder.run((b) => b.services.questions.create(key, input, this.actor(b)));
      return reply.code(201).send(created);
    });
    app.get('/api/db/questions/:id', async (request) => {
      const { id } = request.params as { id: string };
      return this.holder.run((b) => b.services.questions.get(id));
    });
    app.patch('/api/db/questions/:id', async (request, reply) => {
      const { id } = request.params as { id: string };
      const body = this.input.body(request.body);
      const rev = this.rev(body);
      const changes = {
        title: this.input.optionalText(body, 'title'),
        description: this.input.optionalText(body, 'description'),
        tags: this.input.optionalList(body, 'tags'),
        occasion: this.input.nullableText(body, 'occasion'),
      };
      return this.holder.run((b) => this.withConflict(b, id, reply, () => b.services.questions.update(id, changes, this.actor(b), { rev })));
    });
    app.put('/api/db/questions/:id/tickets', async (request, reply) => {
      const { id } = request.params as { id: string };
      const body = this.input.body(request.body);
      const rev = this.rev(body);
      const ticketIds = this.list(body, 'ticketIds');
      return this.holder.run((b) => this.withConflict(b, id, reply, () => b.services.questions.setTickets(id, ticketIds, this.actor(b), { rev })));
    });
    app.put('/api/db/questions/:id/depends', async (request, reply) => {
      const { id } = request.params as { id: string };
      const body = this.input.body(request.body);
      const rev = this.rev(body);
      const dependsOn = this.list(body, 'dependsOn');
      return this.holder.run((b) => this.withConflict(b, id, reply, () => b.services.questions.setDepends(id, dependsOn, this.actor(b), { rev })));
    });
    app.post('/api/db/questions/:id/comments', async (request, reply) => {
      const { id } = request.params as { id: string };
      const text = this.input.text(this.input.body(request.body), 'body');
      const detail = await this.holder.run((b) => b.services.questions.comment(id, text, this.actor(b)));
      return reply.code(201).send(detail);
    });
    app.post('/api/db/questions/:id/decision', async (request, reply) => {
      const { id } = request.params as { id: string };
      const body = this.input.body(request.body);
      const rev = this.rev(body);
      const input = { option: this.input.optionalText(body, 'option'), text: this.input.optionalText(body, 'text'), reason: this.input.optionalText(body, 'reason') };
      return this.holder.run((b) => this.withConflict(b, id, reply, () => b.services.questions.answer(id, input, this.actor(b), { rev })));
    });
    app.post('/api/db/questions/:id/discard', async (request, reply) => {
      const { id } = request.params as { id: string };
      const body = this.input.body(request.body);
      const rev = this.rev(body);
      const reason = this.input.optionalText(body, 'reason');
      return this.holder.run((b) => this.withConflict(b, id, reply, () => b.services.questions.discard(id, reason, this.actor(b), { rev })));
    });
    app.post('/api/db/questions/:id/reopen', async (request, reply) => {
      const { id } = request.params as { id: string };
      const body = this.input.body(request.body);
      const rev = this.rev(body);
      const reason = this.input.optionalText(body, 'reason');
      return this.holder.run((b) => this.withConflict(b, id, reply, () => b.services.questions.reopen(id, reason, this.actor(b), { rev })));
    });
  }

  // Der Mensch der Oberfläche
  private actor(b: BoardBackend): BoardQuestionActor {
    return { ...b.actor(), kind: 'human', session: null };
  }

  // Gelesene Version; Pflicht bei allen Änderungen mit Konfliktprüfung
  private rev(body: Body): number {
    const rev = this.input.optionalInt(body, 'rev');
    if (rev === undefined) throw new HarnessError('rev-missing', 'Zum Ändern gehört die gelesene Version (rev).');
    return rev;
  }

  // Pflichtliste von Texten (darf leer sein)
  private list(body: Body, key: string): string[] {
    const value = this.input.optionalList(body, key);
    if (!value) throw new HarnessError('input-invalid', `Feld „${key}“ fehlt oder ist keine Liste von Texten.`);
    return value;
  }

  // Einzelner Text-Parameter der Liste; mehrfach angegeben oder kein Text ist ungültig
  private param(query: Record<string, unknown>, key: string): string | undefined {
    const value = query[key];
    if (value === undefined) return undefined;
    if (typeof value !== 'string') throw new HarnessError('input-invalid', `Parameter „${key}“ muss genau einmal als Text angegeben werden.`);
    return value;
  }

  // Zustandsfilter der Liste; Standard: alle
  private state(raw: string | undefined): QuestionState | 'all' {
    const value = raw || 'all';
    if (value !== 'all' && !QUESTION_STATES.includes(value as QuestionState)) {
      throw new HarnessError('input-invalid', `Filter „${value}“ ist ungültig. Erlaubt: open, answered, discarded, all.`);
    }
    return value as QuestionState | 'all';
  }

  // Bei veralteter Version: 409 mit dem aktuellen Stand der Frage, sonst das Ergebnis der Arbeit
  private async withConflict(b: BoardBackend, id: string, reply: FastifyReply, work: () => Promise<BoardQuestionDetail>): Promise<BoardQuestionDetail | FastifyReply> {
    try {
      return await work();
    } catch (error) {
      if (error instanceof HarnessError && error.code === 'conflict') {
        return reply.code(409).send({ code: 'conflict', message: error.message, current: await b.services.questions.get(id) });
      }
      throw error;
    }
  }
}
