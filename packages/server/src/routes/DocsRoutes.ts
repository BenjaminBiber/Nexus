import type { FastifyInstance } from 'fastify';
import { HarnessError } from '@harness/core';
import type { DocQuestionRequest, DocQuestionService } from '../docs/DocQuestionService.js';
import type { DocRevisionService, ReviseDocInput } from '../docs/DocRevisionService.js';
import type { DocsRootResolver } from '../docs/DocsRootResolver.js';
import type { DocTicketRequest, DocTicketService } from '../docs/DocTicketService.js';
import type { DocsService, WriteSectionInput } from '../docs/DocsService.js';
import type { FileOpener } from '../docs/FileOpener.js';
import type { TicketDraftRequest, TicketDraftService } from '../docs/TicketDraftService.js';
import type { ProjectService } from '../projects/ProjectService.js';

// GET …/docs, …/docs/file, …/docs/search, PUT …/docs/section, POST …/docs/revise und POST …/docs/ticket-drafts: Superpowers-Dokumente,
// Glossar und ADRs aus dem Repo des Boards lesen, durchsuchen, Abschnitte schreiben, Bereiche von Claude umschreiben lassen
// und zu Ticket-Entwürfen zerlegen. POST …/docs/tickets legt geprüfte Entwürfe als Tickets an,
// GET …/docs/tickets nennt die Tickets eines Dokuments samt der Stelle, aus der sie entstanden sind. POST …/docs/question-drafts
// liest die offenen Fragen heraus, POST …/docs/questions legt sie als Board-Fragen an. POST …/docs/open öffnet
// das Dokument im Explorer oder in VS Code.
export class DocsRoutes {
  constructor(
    private readonly projects: ProjectService,
    private readonly docs: DocsService,
    private readonly resolver: DocsRootResolver,
    private readonly revisions: DocRevisionService,
    private readonly drafts: TicketDraftService,
    private readonly tickets: DocTicketService,
    private readonly opener: FileOpener,
    private readonly questions: DocQuestionService,
  ) {}

  // Meldet die Routen an
  register(app: FastifyInstance): void {
    app.get('/api/projects/:pid/docs', async (request) => {
      const { project } = await this.projects.open((request.params as { pid: string }).pid);
      return this.drafts.flag(this.docs.list(await this.resolver.resolveFor(project.root, (request.query as { board?: string }).board)));
    });
    app.get('/api/projects/:pid/docs/file', async (request) => {
      const { project } = await this.projects.open((request.params as { pid: string }).pid);
      return this.docs.read(await this.resolver.resolveFor(project.root, (request.query as { board?: string }).board), (request.query as { path?: string }).path);
    });
    app.get('/api/projects/:pid/docs/search', async (request) => {
      const { project } = await this.projects.open((request.params as { pid: string }).pid);
      return this.docs.search(await this.resolver.resolveFor(project.root, (request.query as { board?: string }).board), (request.query as { q?: string }).q ?? '');
    });
    app.post('/api/projects/:pid/docs/ticket-drafts', async (request) => {
      const pid = (request.params as { pid: string }).pid;
      const { project } = await this.projects.open(pid);
      const source = await this.resolver.resolveFor(project.root, (request.query as { board?: string }).board);
      return this.drafts.create(source, pid, (request.body ?? {}) as TicketDraftRequest);
    });
    // Bricht die laufende Erzeugung von Ticket-Entwürfen des Projekts ab (beendet Claude)
    app.delete('/api/projects/:pid/docs/ticket-drafts', async (request) => {
      const pid = (request.params as { pid: string }).pid;
      await this.projects.open(pid);
      return { cancelled: this.drafts.cancel(pid) };
    });
    // Gespeicherte Entwürfe eines Dokuments lesen ({ result: null } ohne), bearbeitete speichern oder verwerfen
    app.get('/api/projects/:pid/docs/ticket-drafts/saved', async (request) => {
      const { project } = await this.projects.open((request.params as { pid: string }).pid);
      const source = await this.resolver.resolveFor(project.root, (request.query as { board?: string }).board);
      return { result: await this.drafts.saved(source, (request.query as { path?: string }).path) };
    });
    app.put('/api/projects/:pid/docs/ticket-drafts/saved', async (request) => {
      const { project } = await this.projects.open((request.params as { pid: string }).pid);
      const source = await this.resolver.resolveFor(project.root, (request.query as { board?: string }).board);
      const body = (request.body ?? {}) as { path?: string; drafts?: unknown };
      return { result: await this.drafts.update(source, body.path, body.drafts) };
    });
    app.delete('/api/projects/:pid/docs/ticket-drafts/saved', async (request) => {
      const { project } = await this.projects.open((request.params as { pid: string }).pid);
      const source = await this.resolver.resolveFor(project.root, (request.query as { board?: string }).board);
      await this.drafts.discard(source, (request.query as { path?: string }).path);
      return { ok: true };
    });
    // Tickets, die aus diesem Dokument entstanden sind, samt Überschrift und Zeilen ihrer Quelle
    app.get('/api/projects/:pid/docs/tickets', async (request) => {
      const { project } = await this.projects.open((request.params as { pid: string }).pid);
      const source = await this.resolver.resolveFor(project.root, (request.query as { board?: string }).board);
      return this.tickets.listFor(source, (request.query as { path?: string }).path);
    });
    // Legt die geprüften Entwürfe als Tickets auf dem Board des Ordners an
    app.post('/api/projects/:pid/docs/tickets', async (request) => {
      const { project } = await this.projects.open((request.params as { pid: string }).pid);
      const source = await this.resolver.resolveFor(project.root, (request.query as { board?: string }).board);
      const body = (request.body ?? {}) as DocTicketRequest;
      const result = await this.tickets.create(source, body);
      // Angelegt: die gespeicherten Entwürfe haben ihren Zweck erfüllt
      await this.drafts.discard(source, body.path).catch(() => {});
      return result;
    });
    // Lässt Claude die offenen Fragen des Dokuments herauslesen (Entwürfe, nicht gespeichert)
    app.post('/api/projects/:pid/docs/question-drafts', async (request) => {
      const pid = (request.params as { pid: string }).pid;
      const { project } = await this.projects.open(pid);
      const source = await this.resolver.resolveFor(project.root, (request.query as { board?: string }).board);
      return this.questions.drafts(source, pid, ((request.body ?? {}) as { path?: string }).path);
    });
    // Bricht die laufende Erzeugung von Fragen-Entwürfen des Projekts ab (beendet Claude)
    app.delete('/api/projects/:pid/docs/question-drafts', async (request) => {
      const pid = (request.params as { pid: string }).pid;
      await this.projects.open(pid);
      return { cancelled: this.questions.cancel(pid) };
    });
    // Legt die gewählten Entwürfe als Fragen im Projekt des Boards an
    app.post('/api/projects/:pid/docs/questions', async (request) => {
      const { project } = await this.projects.open((request.params as { pid: string }).pid);
      const source = await this.resolver.resolveFor(project.root, (request.query as { board?: string }).board);
      return this.questions.create(source, (request.body ?? {}) as DocQuestionRequest);
    });
    // Öffnet das Dokument auf diesem Rechner im Explorer (markiert) oder in VS Code
    app.post('/api/projects/:pid/docs/open', async (request) => {
      const { project } = await this.projects.open((request.params as { pid: string }).pid);
      const source = await this.resolver.resolveFor(project.root, (request.query as { board?: string }).board);
      const body = (request.body ?? {}) as { path?: unknown; target?: unknown };
      if (body.target !== 'explorer' && body.target !== 'vscode') {
        throw new HarnessError('request-invalid', 'Ziel muss „explorer“ oder „vscode“ sein.');
      }
      await this.opener.open(this.docs.fullPath(source, typeof body.path === 'string' ? body.path : undefined), body.target);
      return { ok: true };
    });
    app.put('/api/projects/:pid/docs/section', async (request) => {
      const { project } = await this.projects.open((request.params as { pid: string }).pid);
      return this.docs.writeSection(await this.resolver.resolveFor(project.root, (request.query as { board?: string }).board), this.validSection(request.body));
    });
    app.post('/api/projects/:pid/docs/revise', async (request) => {
      const { project } = await this.projects.open((request.params as { pid: string }).pid);
      return this.revisions.revise(await this.resolver.resolveFor(project.root, (request.query as { board?: string }).board), this.validRevise(request.body));
    });
  }

  // Prüft den Body der Umschreib-Route; der Zeilenbereich selbst wird erst am gelesenen Dokument geprüft
  private validRevise(body: unknown): ReviseDocInput {
    const b = (body ?? {}) as Partial<ReviseDocInput>;
    const path = typeof b.path === 'string' ? b.path.trim() : '';
    const expectedHash = typeof b.expectedHash === 'string' ? b.expectedHash.trim() : '';
    const instruction = typeof b.instruction === 'string' ? b.instruction.trim() : '';
    if (!path || !expectedHash || !instruction) {
      throw new HarnessError('request-invalid', 'Zum Umschreiben braucht es Pfad, Hash und eine Anweisung.');
    }
    if (!Number.isInteger(b.startLine) || !Number.isInteger(b.endLine) || (b.startLine as number) < 1 || (b.endLine as number) < (b.startLine as number)) {
      throw new HarnessError('doc-range-invalid', 'Anfang und Ende müssen ganze Zeilennummern ab 1 sein, das Ende nicht vor dem Anfang.');
    }
    return { path, expectedHash, instruction, startLine: b.startLine as number, endLine: b.endLine as number };
  }

  // Prüft den Body der Schreibroute; Zeilenfelder getrennt, damit der Client 400 statt 422 sieht
  private validSection(body: unknown): WriteSectionInput {
    const b = (body ?? {}) as Partial<WriteSectionInput>;
    const path = typeof b.path === 'string' ? b.path.trim() : '';
    const expectedHash = typeof b.expectedHash === 'string' ? b.expectedHash.trim() : '';
    // Ein leerer Inhalt ist erlaubt: so lässt sich ein Abschnitt leeren
    if (!path || !expectedHash || typeof b.content !== 'string') {
      throw new HarnessError('request-invalid', 'Zum Schreiben braucht es Pfad, Hash und Inhalt.');
    }
    if (!Number.isInteger(b.startLine) || !Number.isInteger(b.endLine) || (b.startLine as number) < 1 || (b.endLine as number) < (b.startLine as number)) {
      throw new HarnessError('doc-range-invalid', 'Anfang und Ende müssen ganze Zeilennummern ab 1 sein, das Ende nicht vor dem Anfang.');
    }
    return { path, expectedHash, content: b.content, startLine: b.startLine as number, endLine: b.endLine as number };
  }
}
