import { HarnessError, type BoardStepGuard, type BoardTicketPatch } from '@harness/core';
import type { FastifyInstance, FastifyReply } from 'fastify';
import type { BoardBackend } from '../dbboard/BoardBackend.js';
import type { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import type { TicketTimelineAssembler } from '../dbboard/TicketTimelineAssembler.js';
import { DbInput, type Body } from '../dbboard/DbInput.js';

// Höchstzahl der Tickets, die ein Aufruf von einem anderen Board holt
const MAX_IMPORT_IDS = 200;

// /api/db/…/tickets: Tickets der Board-Datenbank anlegen, lesen, ändern (mit Version), verschieben, löschen, kommentieren
// und von anderen Boards desselben Projekts holen.
// Schreibaktionen gelten als Aktion des Nutzers (byUser): z. B. ist „Nicht mehr nötig“ erlaubt.
export class DbTicketRoutes {
  private readonly input = new DbInput();

  constructor(private readonly holder: BoardRuntimeHolder, private readonly timeline: TicketTimelineAssembler) {}

  // Meldet die Routen an
  register(app: FastifyInstance): void {
    app.post('/api/db/boards/:id/tickets', async (request, reply) => {
      const boardId = this.input.id((request.params as { id: string }).id, 'Board');
      const body = this.input.body(request.body);
      const input = {
        title: this.input.text(body, 'title'),
        description: this.input.optionalText(body, 'description'),
        status: this.input.optionalStatus(body, 'status'),
        priority: this.input.optionalPriority(body, 'priority'),
        labels: this.input.optionalList(body, 'labels'),
        dependsOn: this.input.optionalList(body, 'dependsOn'),
        acceptance: this.input.optionalText(body, 'acceptance'),
        needsPlanning: this.input.optionalBool(body, 'needsPlanning'),
        targetBranch: this.input.nullableText(body, 'targetBranch'),
      };
      const ticket = await this.holder.run((b) => b.services.tickets.create(boardId, input, b.actor(), { byUser: true }));
      return reply.code(201).send(ticket);
    });
    // Holt Tickets von anderen Boards desselben Projekts auf dieses Board (alles oder nichts)
    app.post('/api/db/boards/:id/import', async (request) => {
      const boardId = this.input.id((request.params as { id: string }).id, 'Board');
      const ticketIds = this.input.requiredList(this.input.body(request.body), 'ticketIds', MAX_IMPORT_IDS);
      const moved = await this.holder.run((b) => b.services.tickets.moveToBoard(ticketIds, boardId, b.actor(), { byUser: true }));
      return { moved };
    });
    app.get('/api/db/tickets/:id', async (request) => {
      const { id } = request.params as { id: string };
      return this.holder.run((b) => b.services.tickets.get(id));
    });
    // Zeitleiste: Verlauf, Kommentare, Fragen, Läufe und Commits in Zeitfolge (lokale Quellen nur auf diesem Rechner)
    app.get('/api/db/tickets/:id/timeline', async (request) => {
      const { id } = request.params as { id: string };
      return this.timeline.get(id);
    });
    app.patch('/api/db/tickets/:id', async (request, reply) => {
      const { id } = request.params as { id: string };
      const body = this.input.body(request.body);
      const rev = this.input.optionalInt(body, 'rev');
      if (rev === undefined) throw new HarnessError('rev-missing', 'Zum Ändern gehört die gelesene Version (rev).');
      const patch = this.patch(body);
      return this.holder.run((b) => this.withConflict(b, id, reply, async () => {
        await b.services.tickets.update(id, patch, b.actor(), { rev, byUser: true });
        return b.services.tickets.get(id);
      }));
    });
    app.delete('/api/db/tickets/:id', async (request, reply) => {
      const { id } = request.params as { id: string };
      await this.holder.run((b) => b.services.tickets.delete(id, b.actor()));
      return reply.code(204).send();
    });
    app.post('/api/db/tickets/:id/move', async (request, reply) => {
      const { id } = request.params as { id: string };
      const body = this.input.body(request.body);
      const status = this.input.optionalStatus(body, 'status');
      if (!status) throw new HarnessError('status-missing', 'Für das Verschieben fehlt der Status.');
      const rev = this.input.optionalInt(body, 'rev');
      const acceptance = this.input.optionalText(body, 'acceptance');
      return this.holder.run((b) => this.withConflict(b, id, reply, () =>
        b.services.tickets.move(id, status, b.actor(), { rev, acceptance, byUser: true })));
    });
    app.post('/api/db/tickets/:id/comments', async (request, reply) => {
      const { id } = request.params as { id: string };
      const text = this.input.text(this.input.body(request.body), 'body');
      const detail = await this.holder.run(async (b) => {
        await b.services.comments.add(id, text, b.actor());
        return b.services.tickets.get(id);
      });
      return reply.code(201).send(detail);
    });
    // Hakt einen Schritt ab oder nimmt den Haken zurück (Nummer ab 1). Alle Schritt-Routen nehmen optional
    // rev (Ticket-Version) und expectText (bisheriger Text des Schritts); bei Abweichung 409 mit current.
    app.post('/api/db/tickets/:id/steps/:n', async (request, reply) => {
      const { id, n } = request.params as { id: string; n: string };
      const number = this.stepNumber(n);
      const body = this.input.body(request.body);
      const done = this.input.optionalBool(body, 'done');
      if (done === undefined) throw new HarnessError('input-invalid', 'Feld „done“ fehlt (true oder false).');
      const guard = this.stepGuard(body);
      return this.holder.run((b) => this.withConflict(b, id, reply, () => b.services.tickets.setStep(id, number, done, b.actor(), guard)));
    });
    // Legt einen Schritt an (ans Ende oder an Nummer „at“); Antwort: Ticketdetail
    app.post('/api/db/tickets/:id/steps', async (request, reply) => {
      const { id } = request.params as { id: string };
      const body = this.input.body(request.body);
      const text = this.input.text(body, 'text');
      const detail = this.input.nullableText(body, 'detail');
      const at = this.input.optionalInt(body, 'at');
      const guard = this.stepGuard(body);
      return this.holder.run((b) => this.withConflict(b, id, reply, async () => {
        const result = await b.services.tickets.addStep(id, text, b.actor(), { detail, at, ...guard });
        reply.code(201);
        return result;
      }));
    });
    // Ändert Text und/oder Anleitung eines Schritts (detail null oder leer entfernt die Anleitung)
    app.patch('/api/db/tickets/:id/steps/:n', async (request, reply) => {
      const { id, n } = request.params as { id: string; n: string };
      const number = this.stepNumber(n);
      const body = this.input.body(request.body);
      const change = { text: this.input.optionalText(body, 'text'), detail: this.input.nullableText(body, 'detail') };
      const guard = this.stepGuard(body);
      return this.holder.run((b) => this.withConflict(b, id, reply, () => b.services.tickets.editStep(id, number, change, b.actor(), guard)));
    });
    // Löscht einen Schritt; die folgenden rücken nach
    app.delete('/api/db/tickets/:id/steps/:n', async (request, reply) => {
      const { id, n } = request.params as { id: string; n: string };
      const number = this.stepNumber(n);
      const guard = this.stepGuard(this.input.body(request.body));
      return this.holder.run((b) => this.withConflict(b, id, reply, () => b.services.tickets.removeStep(id, number, b.actor(), guard)));
    });
    // Verschiebt einen Schritt auf Nummer „to“
    app.post('/api/db/tickets/:id/steps/:n/move', async (request, reply) => {
      const { id, n } = request.params as { id: string; n: string };
      const from = this.stepNumber(n);
      const body = this.input.body(request.body);
      const to = this.input.optionalInt(body, 'to');
      if (to === undefined) throw new HarnessError('input-invalid', 'Feld „to“ fehlt (Zielnummer ab 1).');
      const guard = this.stepGuard(body);
      return this.holder.run((b) => this.withConflict(b, id, reply, () => b.services.tickets.moveStep(id, from, to, b.actor(), guard)));
    });
  }

  // Optionaler Schutz einer Schritt-Änderung: Ticket-Version (rev) und bisheriger Text des Schritts (expectText)
  private stepGuard(body: Body): BoardStepGuard {
    return { rev: this.input.optionalInt(body, 'rev'), expectText: this.input.optionalText(body, 'expectText') };
  }

  // Schrittnummer aus der Adresse (ab 1)
  private stepNumber(raw: string): number {
    const number = Number(raw);
    if (!Number.isInteger(number) || number < 1) throw new HarnessError('input-invalid', `„${raw}“ ist keine Schrittnummer (ab 1).`);
    return number;
  }

  // Bei veralteter Version: 409 mit dem aktuellen Stand des Tickets, sonst das Ergebnis der Arbeit
  private async withConflict<T>(b: BoardBackend, id: string, reply: FastifyReply, work: () => Promise<T>): Promise<T | FastifyReply> {
    try {
      return await work();
    } catch (error) {
      if (error instanceof HarnessError && error.code === 'conflict') {
        return reply.code(409).send({ code: 'conflict', message: error.message, current: await b.services.tickets.get(id) });
      }
      throw error;
    }
  }

  // Änderungswunsch aus dem Body; nicht genannte Felder bleiben undefined. Branch und Worktree lassen sich nur gemeinsam leeren
  // (branch: null oder worktree: null), nie setzen – das tun nur start und der Dispatcher
  private patch(body: Body): BoardTicketPatch {
    for (const key of ['branch', 'worktree']) {
      if (body[key] !== undefined && body[key] !== null) {
        throw new HarnessError('input-invalid', 'Branch und Worktree lassen sich hier nur leeren (null), nicht setzen.');
      }
    }
    const clear = body.branch === null || body.worktree === null;
    return {
      ...(clear ? { branch: null, worktree: null } : {}),
      title: this.input.optionalText(body, 'title'),
      description: this.input.optionalText(body, 'description'),
      plan: this.input.optionalText(body, 'plan'),
      handbook: this.input.optionalText(body, 'handbook'),
      diagram: this.input.optionalText(body, 'diagram'),
      acceptance: this.input.optionalText(body, 'acceptance'),
      status: this.input.optionalStatus(body, 'status'),
      priority: this.input.optionalPriority(body, 'priority'),
      labels: this.input.optionalList(body, 'labels'),
      assignee: this.input.nullableText(body, 'assignee'),
      blockedReason: this.input.nullableText(body, 'blockedReason'),
      targetBranch: this.input.nullableText(body, 'targetBranch'),
      needsPlanning: this.input.optionalBool(body, 'needsPlanning'),
      dependsOn: this.input.optionalList(body, 'dependsOn'),
      dependsOnAdd: this.input.optionalList(body, 'dependsOnAdd'),
      dependsOnRemove: this.input.optionalList(body, 'dependsOnRemove'),
    };
  }
}
