import {
  DispatchConfigNormalizer, DispatchConfigStore, HarnessError, PROMPT_KINDS,
  type DispatchConfig, type DispatchPromptKind, type HarnessHome, type RawDispatchConfig,
} from '@harness/core';
import type { FastifyInstance } from 'fastify';
import type { DispatchService } from '../dispatch/DispatchService.js';
import { PromptCatalog, type DispatchPromptInfo } from '../dispatch/PromptCatalog.js';

export interface DispatchConfigView {
  file: string;
  exists: boolean;
  raw: RawDispatchConfig;
  config: DispatchConfig | null;
  error: string | null;
  /** Standardvorlage, Platzhalter und Pflichtteile je Laufart – unabhängig von der Datei */
  promptInfo: DispatchPromptInfo[];
}

// /api/dispatch: Einstellung, Zustand, Steuerung, „Erneut mergen“ und „Abnehmen“ (Zustand plus `result` für das Ticket), „Recherche starten“
// (Zustand plus `started` für den Lauf), Läufe eines Tickets bzw. einer Frage (zerlegt für den Reiter „Agent“) und Protokolle.
export class DispatchRoutes {
  private readonly store: DispatchConfigStore;
  private readonly prompts = new PromptCatalog();

  constructor(private readonly service: DispatchService, home: HarnessHome) {
    this.store = new DispatchConfigStore(home);
  }

  // Meldet die Routen an
  register(app: FastifyInstance): void {
    app.get('/api/dispatch/config', async () => this.config());
    app.put('/api/dispatch/config', async (request) => {
      const body = request.body;
      if (!body || typeof body !== 'object' || Array.isArray(body)) throw new HarnessError('input-invalid', 'Die Einstellung muss ein Objekt sein.');
      const previous = (await this.config()).config;
      const before = previous?.research.auto ?? false;
      const planBefore = previous?.planning.auto ?? false;
      await this.store.write(body as RawDispatchConfig);
      const view = await this.config();
      // Recherche-Automatik ein- oder ausgeschaltet: neuer Zeitpunkt, ab dem neue Fragen zählen
      const after = view.config?.research.auto ?? false;
      if (after !== before) await this.service.researchAutoSwitched(after);
      // Planungs-Automatik ein- oder ausgeschaltet: neuer Zeitpunkt, ab dem neue Tickets zählen
      const planAfter = view.config?.planning.auto ?? false;
      if (planAfter !== planBefore) await this.service.planningAutoSwitched(planAfter);
      return view;
    });
    app.post('/api/dispatch/prompts/preview', async (request) => {
      const body = request.body;
      if (!body || typeof body !== 'object' || Array.isArray(body)) throw new HarnessError('input-invalid', 'Die Anfrage muss ein Objekt sein.');
      const { kind, template, id } = body as { kind?: unknown; template?: unknown; id?: unknown };
      if (typeof kind !== 'string' || !(PROMPT_KINDS as readonly string[]).includes(kind)) {
        throw new HarnessError('input-invalid', `kind muss ${PROMPT_KINDS.join(', ')} sein.`);
      }
      if (template !== undefined && template !== null && typeof template !== 'string') throw new HarnessError('input-invalid', 'template muss eine Vorlage als Text sein.');
      if (id !== undefined && id !== null && (typeof id !== 'string' || !id.trim())) throw new HarnessError('input-invalid', 'id muss eine Ticket- oder Fragen-ID sein.');
      return this.service.previewPrompt({ kind: kind as DispatchPromptKind, template: template ?? null, id: (id as string | null | undefined) ?? null });
    });
    app.get('/api/dispatch/status', async (request) => {
      const boardId = this.boardId((request.query as Record<string, unknown>).boardId);
      await this.service.requireBoard(boardId);
      return this.service.status(boardId);
    });
    app.post('/api/dispatch/run', async (request) => {
      const boardId = await this.bodyBoard(request.body);
      await this.service.tick();
      return this.service.status(boardId);
    });
    app.post('/api/dispatch/pause', async (request) => {
      const boardId = await this.bodyBoard(request.body);
      const minutes = (request.body as { minutes?: unknown } | undefined)?.minutes ?? 60;
      if (!Number.isInteger(minutes) || (minutes as number) < 1 || (minutes as number) > 1440) {
        throw new HarnessError('input-invalid', 'minutes muss eine ganze Zahl von 1 bis 1440 sein.');
      }
      await this.service.pause(minutes as number);
      return this.service.status(boardId);
    });
    app.post('/api/dispatch/resume', async (request) => {
      const boardId = await this.bodyBoard(request.body);
      await this.service.resume();
      return this.service.status(boardId);
    });
    app.post('/api/dispatch/enable', async (request) => {
      const boardId = await this.bodyBoard(request.body);
      await this.service.setEnabled(true);
      return this.service.status(boardId);
    });
    app.post('/api/dispatch/disable', async (request) => {
      const boardId = await this.bodyBoard(request.body);
      await this.service.setEnabled(false);
      return this.service.status(boardId);
    });
    app.post('/api/dispatch/merge/:id', async (request) => {
      const boardId = await this.bodyBoard(request.body);
      const { result } = await this.service.retryMerge((request.params as { id: string }).id);
      return { ...(await this.service.status(boardId)), result };
    });
    app.post('/api/dispatch/accept/:id', async (request) => {
      const boardId = await this.bodyBoard(request.body);
      const by = (request.body as { by?: unknown } | undefined)?.by;
      if (by !== undefined && by !== null && typeof by !== 'string') throw new HarnessError('input-invalid', 'by muss ein Name sein.');
      const { result } = await this.service.accept((request.params as { id: string }).id, by ?? undefined);
      return { ...(await this.service.status(boardId)), result };
    });
    app.get('/api/dispatch/review/:id', async (request) => this.service.review((request.params as { id: string }).id));
    app.get('/api/dispatch/review/:id/file', async (request) => {
      const path = (request.query as Record<string, unknown>).path;
      if (typeof path !== 'string' || !path) throw new HarnessError('input-invalid', 'path (Datei aus der Liste) fehlt.');
      return this.service.reviewFile((request.params as { id: string }).id, path);
    });
    app.post('/api/dispatch/return/:id', async (request) => {
      const boardId = await this.bodyBoard(request.body);
      const body = request.body as { comment?: unknown; by?: unknown } | undefined;
      if (typeof body?.comment !== 'string') throw new HarnessError('input-invalid', 'comment (Rückmeldung an den Agenten) fehlt.');
      if (body.by !== undefined && body.by !== null && typeof body.by !== 'string') throw new HarnessError('input-invalid', 'by muss ein Name sein.');
      const { result } = await this.service.returnToAgent((request.params as { id: string }).id, body.comment, body.by ?? undefined);
      return { ...(await this.service.status(boardId)), result };
    });
    app.get('/api/dispatch/research/:questionId', async (request) => this.service.researchStatus((request.params as { questionId: string }).questionId));
    app.post('/api/dispatch/research/:questionId', async (request) => {
      const boardId = await this.bodyBoard(request.body);
      const started = await this.service.startResearch((request.params as { questionId: string }).questionId);
      return { ...(await this.service.status(boardId)), started };
    });
    app.get('/api/dispatch/plan/:ticketId', async (request) => this.service.planningStatus((request.params as { ticketId: string }).ticketId));
    app.post('/api/dispatch/plan/:ticketId', async (request) => {
      const boardId = await this.bodyBoard(request.body);
      const started = await this.service.startPlanning((request.params as { ticketId: string }).ticketId);
      return { ...(await this.service.status(boardId)), started };
    });
    app.get('/api/dispatch/runs', async (request) => {
      const ticket = (request.query as Record<string, unknown>).ticket;
      if (typeof ticket !== 'string' || !ticket.trim()) throw new HarnessError('input-invalid', 'ticket (Ticket- oder Fragen-ID) fehlt.');
      return this.service.ticketRuns(ticket);
    });
    app.get('/api/dispatch/runs/:name', async (request) => this.service.runLog((request.params as { name: string }).name));
    app.get('/api/dispatch/logs', async () => this.service.stores.log.list());
    app.get('/api/dispatch/logs/:name', async (request, reply) => {
      const text = await this.service.stores.log.read((request.params as { name: string }).name);
      return reply.type('text/plain; charset=utf-8').send(text);
    });
  }

  // Einstellung für die Oberfläche; eine kaputte Datei liefert das Beispiel mit Fehlermeldung
  private async config(): Promise<DispatchConfigView> {
    // auch im Fehlerfall dabei, damit die Oberfläche die Vorlagen bei kaputter dispatch.yaml noch anzeigen kann
    const promptInfo = this.prompts.list();
    try {
      const read = await this.store.read();
      return { file: this.store.file, exists: read.exists, raw: read.raw, config: read.config, error: null, promptInfo };
    } catch (error) {
      return { file: this.store.file, exists: true, raw: new DispatchConfigNormalizer().example(), config: null, error: (error as Error).message, promptInfo };
    }
  }

  // Board-ID aus dem Body; vor jeder Aktion geprüft (Form und Existenz), damit eine ungültige Eingabe nichts verändert
  private async bodyBoard(body: unknown): Promise<number | undefined> {
    const boardId = this.boardId((body as { boardId?: unknown } | undefined)?.boardId);
    await this.service.requireBoard(boardId);
    return boardId;
  }

  // Board-ID aus Abfrage oder Body; fehlend → undefined
  private boardId(value: unknown): number | undefined {
    if (value === undefined || value === null || value === '') return undefined;
    const id = Number(value);
    if (!Number.isInteger(id) || id < 1) throw new HarnessError('id-invalid', `„${String(value)}“ ist keine Board-ID.`);
    return id;
  }
}
