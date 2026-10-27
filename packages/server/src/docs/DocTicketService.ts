import { HarnessError, MAX_SPEC_LENGTH, TicketDraftValidator, type NewBoardTicketInput, type TicketDraft } from '@harness/core';
import { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import { formatSource, parseSource } from './DocTicketSource.js';
import type { DocsService } from './DocsService.js';
import type { DocsSource } from './DocsRootResolver.js';

export interface DocTicketRequest {
  path?: string;
  drafts?: TicketDraft[];
}

/** Ticket eines Dokuments samt Stelle, aus der es entstanden ist; ohne Quellenzeile bleibt `heading` null */
export interface DocTicketRef {
  id: string;
  title: string;
  status: string;
  column: string;
  heading: string | null;
  startLine: number;
  endLine: number;
}

/** Angelegtes Ticket samt Platz des Entwurfs in der Eingabe */
export interface CreatedDocTicket {
  index: number;
  id: string;
}

/** Übersprungener Entwurf; bei einer Dublette zeigt `existingId` auf das vorhandene Ticket */
export interface SkippedDocTicket {
  index: number;
  reason: string;
  existingId?: string;
}

export interface DocTicketResult {
  created: CreatedDocTicket[];
  skipped: SkippedDocTicket[];
}

// Legt geprüfte Ticket-Entwürfe eines Dokuments der Reihe nach als Tickets auf dem Board des Projekts an.
// Ein einzelner Entwurf, der nicht durchkommt (Dublette, ungültige Felder), wird übersprungen statt den Rest zu verwerfen.
export class DocTicketService {
  private readonly validator = new TicketDraftValidator();

  constructor(
    private readonly docs: DocsService,
    private readonly holder: BoardRuntimeHolder,
  ) {}

  // Anlegen in Eingabereihenfolge: die IDs der Vorgänger werden für `dependsOn` gebraucht
  async create(source: DocsSource, input: DocTicketRequest): Promise<DocTicketResult> {
    const drafts: unknown[] = Array.isArray(input.drafts) ? input.drafts : [];
    if (drafts.length === 0) throw new HarnessError('invalid-input', 'Bitte mindestens einen Entwurf übergeben.');
    const boardId = this.boardId(source);
    // wirft doc-not-found, wenn es den Pfad nicht gibt
    const doc = this.docs.read(source, input.path);
    const docPath = doc.path;
    const lines = doc.content.split(/\r?\n/).length;

    const created: CreatedDocTicket[] = [];
    const skipped: SkippedDocTicket[] = [];
    // 0-basierter Index des Entwurfs → ID des angelegten oder vorhandenen Tickets
    const ids = new Map<number, string>();
    for (const [index, raw] of drafts.entries()) {
      const draft = this.valid(raw, lines);
      if (!draft) {
        skipped.push({ index, reason: 'draft-invalid' });
        continue;
      }
      try {
        const ticket = await this.holder.run((b) => b.services.tickets.create(boardId, this.input(draft, docPath, ids), b.actor()));
        ids.set(index, ticket.id);
        created.push({ index, id: ticket.id });
      } catch (error) {
        // Nur Fachfehler des einzelnen Entwurfs werden übersprungen; ohne Board-Verbindung oder bei einem Programmfehler
        // scheiterten auch alle weiteren, das soll als Fehler ankommen statt als Antwort voller übersprungener Entwürfe
        if (!(error instanceof HarnessError) || BoardRuntimeHolder.isConnectionFailure(error)) throw error;
        const reason = error.code;
        const existingId = this.existingId(error);
        if (existingId) ids.set(index, existingId);
        skipped.push(existingId ? { index, reason, existingId } : { index, reason });
      }
    }
    return { created, skipped };
  }

  // Tickets, die aus diesem Dokument entstanden sind, mit der Stelle aus ihrer Quellenzeile.
  // Ohne Board oder Repo-Pfad bleibt die Liste leer statt zu werfen: die Chips sind Beiwerk, das Dokument bleibt lesbar.
  async listFor(source: DocsSource, path?: string): Promise<DocTicketRef[]> {
    if (source.boardId === null || source.root === null) return [];
    const boardId = source.boardId;
    // wirft doc-not-found, wenn es den Pfad nicht gibt
    const docPath = this.docs.assertDocument(source, path);
    const spec = docPath.length > MAX_SPEC_LENGTH ? docPath.slice(-MAX_SPEC_LENGTH) : docPath;
    const tickets = await this.holder.run((b) => b.services.tickets.list({ boardIds: [boardId] }));
    return tickets
      .filter((t) => t.spec === spec)
      .map((t) => {
        const found = parseSource(t.description ?? '');
        return {
          id: t.id, title: t.title, status: t.status, column: t.column,
          heading: found?.heading ?? null, startLine: found?.startLine ?? 0, endLine: found?.endLine ?? 0,
        };
      })
      .sort((a, b) => a.startLine - b.startLine || a.id.localeCompare(b.id));
  }

  // Prüft einen Entwurf wie die Antwort von Claude (Pflichtfelder, Zeilenbereich); null, wenn er unbrauchbar ist.
  // dependsOn bleibt als Liste von Eingabe-Indizes stehen, Verweise nach vorn oder ins Leere fallen später in dependsOn() weg.
  private valid(raw: unknown, lines: number): TicketDraft | null {
    if (!raw || typeof raw !== 'object') return null;
    try {
      const [draft] = this.validator.check([raw], { lines }).drafts;
      const dependsOn = (raw as { dependsOn?: unknown }).dependsOn;
      return { ...draft, dependsOn: Array.isArray(dependsOn) ? dependsOn.filter((i): i is number => Number.isInteger(i)) : [] };
    } catch {
      return null;
    }
  }

  // Board des Projektordners; ohne Board oder ohne dessen Repo-Pfad lässt sich nichts anlegen
  private boardId(source: DocsSource): number {
    if (source.boardId === null) {
      throw new HarnessError('no-board', 'Zu diesem Ordner gibt es kein Board; Tickets lassen sich erst nach dem Verknüpfen anlegen.');
    }
    if (source.root === null) {
      throw new HarnessError('no-repo-path', `Das Board „${source.boardName ?? ''}“ hat keinen erreichbaren Repo-Pfad; ohne ihn gibt es keine Dokumente, aus denen Tickets entstehen.`);
    }
    return source.boardId;
  }

  // Entwurf in die Ticket-Form: Backlog, Planungsbedarf, Spec = Dokumentpfad, Quelle am Ende der Beschreibung
  private input(draft: TicketDraft, docPath: string, ids: Map<number, string>): NewBoardTicketInput {
    return {
      title: draft.title,
      description: this.description(draft, docPath),
      status: 'backlog',
      needsPlanning: true,
      labels: draft.labels ?? [],
      acceptance: draft.acceptance ?? '',
      steps: (draft.steps ?? []).map((text) => ({ text, done: false })),
      dependsOn: this.dependsOn(draft, ids),
      // Ticket.spec ist auf 200 Zeichen begrenzt; ein längerer Pfad wird hinten abgeschnitten, damit der Dateiname bleibt
      spec: docPath.length > MAX_SPEC_LENGTH ? docPath.slice(-MAX_SPEC_LENGTH) : docPath,
    };
  }

  // Beschreibung mit Quellenzeile; sie nennt Dokument, Überschrift und Zeilenbereich der markierten Stelle
  private description(draft: TicketDraft, docPath: string): string {
    const { heading, startLine, endLine } = draft.source;
    return `${(draft.description ?? '').trimEnd()}\n\n${formatSource(docPath, heading, startLine, endLine)}`;
  }

  // Indizes in die bis hierher bekannten IDs übersetzen; Verweise auf Unbekanntes (etwa nach vorn) fallen weg
  private dependsOn(draft: TicketDraft, ids: Map<number, string>): string[] {
    return (draft.dependsOn ?? []).flatMap((index) => {
      const id = ids.get(index);
      return id ? [id] : [];
    });
  }

  // ID des vorhandenen Tickets aus einer Dublette
  private existingId(error: unknown): string | undefined {
    if (!(error instanceof HarnessError) || error.code !== 'ticket-duplicate') return undefined;
    const existing = (error.details as { existingId?: unknown } | undefined)?.existingId;
    return typeof existing === 'string' ? existing : undefined;
  }
}
