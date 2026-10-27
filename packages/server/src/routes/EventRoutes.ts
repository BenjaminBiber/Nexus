import type { ServerResponse } from 'node:http';
import type { FastifyInstance } from 'fastify';
import type { BoardChangeWatcher, DbQuestionsEvent } from '../events/BoardChangeWatcher.js';
import type { EventHub } from '../events/EventHub.js';
import type { BrowserNoticeHub } from '../notifications/BrowserNoticeHub.js';
import type { SpecDocWatcher } from '../events/SpecDocWatcher.js';
import type { ProjectService } from '../projects/ProjectService.js';

// GET …/events: Ereignisstrom (Server-Sent Events) mit Begrüßung, Änderungsmeldungen und Herzschlag;
// ?dbBoard=<id> meldet zusätzlich Änderungen dieses DB-Boards, ?dbProject=<id> Änderungen an den Fragen dieses Board-Projekts;
// docs-changed meldet Änderungen an den Superpowers-Dokumenten dieses Projekts (ohne Query-Parameter, die pid steht in der URL);
// ?notify=1 meldet einen Tab, der Benachrichtigungen zeigen darf: er bekommt „notice“ statt des Windows-Toasts.
export class EventRoutes {
  private readonly open = new Set<ServerResponse>();

  constructor(
    private readonly projects: ProjectService,
    private readonly hub: EventHub,
    private readonly heartbeatMs: number,
    private readonly boards: BoardChangeWatcher | null = null,
    private readonly questions: BoardChangeWatcher<DbQuestionsEvent> | null = null,
    private readonly specDocs: SpecDocWatcher | null = null,
    private readonly tabs: BrowserNoticeHub | null = null,
  ) {}

  // Meldet die Route an und sorgt dafür, dass offene Ströme beim Beenden geschlossen werden
  register(app: FastifyInstance): void {
    app.get('/api/projects/:pid/events', async (request, reply) => {
      const { pid } = request.params as { pid: string };
      await this.projects.open(pid);
      reply.hijack();
      const raw = reply.raw;
      raw.writeHead(200, {
        'content-type': 'text/event-stream',
        'cache-control': 'no-cache, no-transform',
        connection: 'keep-alive',
        'x-accel-buffering': 'no',
      });
      this.open.add(raw);
      const send = (event: unknown) => raw.write(`data: ${JSON.stringify(event)}\n\n`);
      send({ type: 'hello' });
      const unsubscribe = this.hub.subscribe(pid, send);
      const { dbBoard, dbProject, notify } = request.query as { dbBoard?: string; dbProject?: string; notify?: string };
      const detachTab = this.tabs && notify === '1' ? this.tabs.attach(send) : () => undefined;
      const boardId = Number(dbBoard);
      const projectId = Number(dbProject);
      const unwatchBoard = this.boards && Number.isInteger(boardId) && boardId > 0 ? this.boards.watch(boardId, send) : () => undefined;
      const unwatchQuestions = this.questions && Number.isInteger(projectId) && projectId > 0 ? this.questions.watch(projectId, send) : () => undefined;
      const unwatchDocs = this.specDocs ? this.specDocs.watch(pid, send) : () => undefined;
      const heartbeat = setInterval(() => raw.write(': ping\n\n'), this.heartbeatMs);
      request.raw.on('close', () => {
        clearInterval(heartbeat);
        unsubscribe();
        unwatchBoard();
        unwatchQuestions();
        unwatchDocs();
        detachTab();
        this.open.delete(raw);
      });
    });
    app.addHook('preClose', async () => {
      this.open.forEach((response) => response.end());
      this.hub.close();
      this.boards?.close();
      this.questions?.close();
      this.specDocs?.close();
    });
  }
}
