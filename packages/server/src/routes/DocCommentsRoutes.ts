import type { FastifyInstance } from 'fastify';
import { DocCommentStore, HarnessError, type DocCommentPatch, type NewDocComment } from '@harness/core';
import type { DocsRootResolver } from '../docs/DocsRootResolver.js';
import type { DocsService } from '../docs/DocsService.js';
import type { ProjectService } from '../projects/ProjectService.js';

// …/docs/comments: Kommentare zu Dokumenten lesen, anlegen, ändern und löschen (schreibt nur *.comments.json neben das Dokument).
export class DocCommentsRoutes {
  constructor(
    private readonly projects: ProjectService,
    private readonly docs: DocsService,
    private readonly resolver: DocsRootResolver,
  ) {}

  // Meldet die Routen an
  register(app: FastifyInstance): void {
    app.get('/api/projects/:pid/docs/comments', async (request) => {
      const { store, path } = await this.open(request);
      return store.list(path);
    });
    app.post('/api/projects/:pid/docs/comments', async (request) => {
      const { store, path } = await this.open(request);
      return store.add(path, this.validNew(request.body));
    });
    app.patch('/api/projects/:pid/docs/comments/:id', async (request) => {
      const { store, path } = await this.open(request);
      return store.update(path, (request.params as { id: string }).id, this.validPatch(request.body));
    });
    app.delete('/api/projects/:pid/docs/comments/:id', async (request) => {
      const { store, path } = await this.open(request);
      await store.remove(path, (request.params as { id: string }).id);
      return { ok: true };
    });
  }

  // Projekt öffnen, Quelle auflösen (Board aus der Query oder das des Ordners), Dokumentpfad prüfen und Store im Repo des Boards bereitstellen
  private async open(request: { params: unknown; query: unknown }) {
    const { project } = await this.projects.open((request.params as { pid: string }).pid);
    const source = await this.resolver.resolveFor(project.root, (request.query as { board?: string }).board);
    // Vor jedem Schreibzugriff: ohne Quelle gibt es kein Dokument, zu dem ein Kommentar gehören könnte
    if (!source.root) throw new HarnessError('doc-not-found', 'Zu diesem Projekt gibt es keine Dokumente.');
    const path = this.docs.assertDocument(source, (request.query as { path?: string }).path);
    return { store: new DocCommentStore(source.root), path };
  }

  // Prüft den Body eines neuen Kommentars
  private validNew(body: unknown): NewDocComment {
    const b = (body ?? {}) as Partial<NewDocComment>;
    const text = typeof b.text === 'string' ? b.text.trim() : '';
    const quote = typeof b.quote === 'string' ? b.quote.trim() : '';
    if (!text || !quote) throw new HarnessError('comment-invalid', 'Ein Kommentar braucht einen Text und eine markierte Stelle.');
    return {
      text,
      quote: quote.slice(0, 500),
      heading: typeof b.heading === 'string' ? b.heading : null,
      lines: Array.isArray(b.lines) && b.lines.length === 2 && b.lines.every(Number.isInteger) ? (b.lines as [number, number]) : null,
    };
  }

  // Prüft eine Änderung (nur Text, Status, Antwort)
  private validPatch(body: unknown): DocCommentPatch {
    const b = (body ?? {}) as DocCommentPatch;
    const patch: DocCommentPatch = {};
    if (typeof b.text === 'string' && b.text.trim()) patch.text = b.text.trim();
    if (b.status === 'open' || b.status === 'resolved') patch.status = b.status;
    if (typeof b.resolution === 'string' || b.resolution === null) patch.resolution = b.resolution;
    return patch;
  }
}
