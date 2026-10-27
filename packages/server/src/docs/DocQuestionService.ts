import {
  AnalysisWorkspace, ClaudeInvocation, DispatchConfigStore, HarnessError, MAX_QUESTION_OCCASION_LENGTH, MAX_QUESTION_TITLE_LENGTH,
  type BoardActor, type BoardQuestionActor, type DispatchConfig, type HarnessHome, type TicketDraftSource,
} from '@harness/core';
import { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import { ClaudeResultEnvelope, excerpt } from '../dispatch/ClaudeResultEnvelope.js';
import type { ClaudeRunner } from '../dispatch/SuggestionAnalyzer.js';
import type { DocTicketService } from './DocTicketService.js';
import { formatSource } from './DocTicketSource.js';
import type { DocsService } from './DocsService.js';
import type { DocsSource } from './DocsRootResolver.js';

/** Entwurf einer Board-Frage aus einem Dokument */
export interface QuestionDraft {
  title: string;
  description: string;
  source: TicketDraftSource;
}

export interface QuestionDraftResult {
  drafts: QuestionDraft[];
  dropped: string[];
  costUsd: number | null;
  at: string;
}

/** Entwurf beim Anlegen; mit `answer` wird die Frage gleich entschieden */
export type QuestionDraftInput = QuestionDraft & { answer?: string };

export interface DocQuestionRequest {
  path?: string;
  drafts?: QuestionDraftInput[];
}

/** Angelegte Frage samt Platz des Entwurfs in der Eingabe; `answered` = gleich mit der Antwort entschieden */
export interface CreatedDocQuestion {
  index: number;
  id: string;
  answered: boolean;
  /** Fehlercode, wenn die Frage angelegt wurde, das Entscheiden aber scheiterte (sie bleibt dann offen) */
  answerError?: string;
}

export interface DocQuestionResult {
  created: CreatedDocQuestion[];
  skipped: { index: number; reason: string }[];
}

/** Mehr offene Fragen auf einmal sind nicht mehr überschaubar */
export const MAX_QUESTION_DRAFTS = 20;
const TIMEOUT_MS = 5 * 60_000;
/** Die Erzeugung liest nur den Prompt – keine Shell, keine Dateien, kein Web */
const DENIED_TOOLS = ['Bash', 'Edit', 'Write', 'MultiEdit', 'NotebookEdit', 'WebFetch', 'WebSearch', 'Task'];

const text = (value: unknown): string => (typeof value === 'string' ? value.trim() : '');

// Liest per Claude die offenen Fragen eines Dokuments heraus und legt die gewählten als Board-Fragen im Projekt des Boards an.
// Die Entwürfe werden nicht gespeichert; ein einzelner Entwurf, der beim Anlegen nicht durchkommt, wird übersprungen.
export class DocQuestionService {
  // Laufende Erzeugung je Projekt; über ihren AbortController lässt sie sich abbrechen
  private readonly running = new Map<string, AbortController>();
  private readonly invocation = new ClaudeInvocation();
  private readonly envelope = new ClaudeResultEnvelope();
  private readonly workspace: AnalysisWorkspace;

  constructor(
    private readonly docs: DocsService,
    private readonly claude: ClaudeRunner,
    private readonly home: HarnessHome,
    private readonly holder: BoardRuntimeHolder,
    private readonly tickets: DocTicketService,
    private readonly clock: () => Date = () => new Date(),
  ) {
    this.workspace = new AnalysisWorkspace(home);
  }

  // Erzeugt die Fragen-Entwürfe zu einem Dokument; je Projekt läuft höchstens eine Erzeugung
  async drafts(source: DocsSource, projectId: string, path: string | undefined): Promise<QuestionDraftResult> {
    const doc = this.docs.read(source, path);
    if (this.running.has(projectId)) {
      throw new HarnessError('conflict', 'Für dieses Projekt läuft schon eine Erzeugung von Fragen.', { reason: 'question-drafts-running' });
    }
    const controller = new AbortController();
    this.running.set(projectId, controller);
    try {
      const cfg = await this.config();
      const result = await this.claude.run({
        command: cfg.claude.command, args: this.args(cfg), prompt: this.prompt(doc.content),
        cwd: await this.workspace.dir(), env: this.invocation.env(process.env, cfg, null) as NodeJS.ProcessEnv,
        timeoutMs: TIMEOUT_MS, onOutput: () => {}, signal: controller.signal,
      });
      if (controller.signal.aborted) throw new HarnessError('drafts-cancelled', 'Die Erzeugung wurde abgebrochen.');
      if (result.code === 127) throw new HarnessError('analysis-failed', `Claude-Befehl „${cfg.claude.command}“ nicht gefunden.`);
      if (result.timedOut) throw new HarnessError('analysis-failed', 'Claude hat die Fragen nicht innerhalb von 5 Minuten geliefert.');
      if (result.code !== 0) throw new HarnessError('analysis-failed', `Claude endete mit Exit-Code ${result.code}: ${excerpt(result.output)}`);
      const { text: answer, costUsd } = this.envelope.parse(result.output);
      const { drafts, dropped } = this.check(this.items(answer), doc.content.split(/\r?\n/).length);
      return { drafts, dropped, costUsd, at: this.clock().toISOString() };
    } finally {
      if (this.running.get(projectId) === controller) this.running.delete(projectId);
    }
  }

  // Bricht die laufende Erzeugung des Projekts ab; false, wenn keine läuft
  cancel(projectId: string): boolean {
    const controller = this.running.get(projectId);
    if (!controller) return false;
    controller.abort();
    this.running.delete(projectId);
    return true;
  }

  // Legt die Entwürfe der Reihe nach als Fragen an: Quelle am Ende der Beschreibung, Anlass = Dokument,
  // verknüpft mit den Tickets des Dokuments, die aus derselben Überschrift entstanden sind; mit Antwort gleich entschieden
  async create(source: DocsSource, input: DocQuestionRequest): Promise<DocQuestionResult> {
    const drafts: unknown[] = Array.isArray(input.drafts) ? input.drafts : [];
    if (drafts.length === 0) throw new HarnessError('invalid-input', 'Bitte mindestens eine Frage übergeben.');
    const boardId = this.boardId(source);
    // wirft doc-not-found, wenn es den Pfad nicht gibt
    const doc = this.docs.read(source, input.path);
    const lines = doc.content.split(/\r?\n/).length;
    const projectKey = await this.projectKey(boardId);
    const refs = await this.tickets.listFor(source, input.path);
    const occasion = `Dokument ${doc.path}`.slice(0, MAX_QUESTION_OCCASION_LENGTH);

    const created: CreatedDocQuestion[] = [];
    const skipped: { index: number; reason: string }[] = [];
    for (const [index, raw] of drafts.entries()) {
      const draft = this.valid(raw, lines);
      if (!draft) {
        skipped.push({ index, reason: 'draft-invalid' });
        continue;
      }
      const { heading, startLine, endLine } = draft.source;
      const ticketIds = refs.filter((r) => heading && r.heading === heading).map((r) => r.id);
      try {
        const question = await this.holder.run((b) => b.services.questions.create(projectKey, {
          title: draft.title,
          description: `${draft.description}\n\n${formatSource(doc.path, heading || '(ohne Überschrift)', startLine, endLine)}`,
          occasion, ticketIds,
        }, this.actor(b.actor())));
        created.push(await this.answer(index, question.id, question.rev, text((raw as { answer?: unknown }).answer)));
      } catch (error) {
        // Nur Fachfehler des einzelnen Entwurfs werden übersprungen, alles andere kommt als Fehler an
        if (!(error instanceof HarnessError) || BoardRuntimeHolder.isConnectionFailure(error)) throw error;
        skipped.push({ index, reason: error.code });
      }
    }
    return { created, skipped };
  }

  // Entscheidet die eben angelegte Frage mit der Antwort aus dem Dialog; ohne Antwort bleibt sie offen (für die Recherche).
  // Scheitert nur das Entscheiden, bleibt die Frage angelegt und offen, und der Fehlercode steht am Eintrag.
  private async answer(index: number, id: string, rev: number, answer: string): Promise<CreatedDocQuestion> {
    if (!answer) return { index, id, answered: false };
    try {
      await this.holder.run((b) => b.services.questions.answer(id, { text: answer }, this.actor(b.actor()), { rev }));
      return { index, id, answered: true };
    } catch (error) {
      if (!(error instanceof HarnessError) || BoardRuntimeHolder.isConnectionFailure(error)) throw error;
      return { index, id, answered: false, answerError: error.code };
    }
  }

  // Der Mensch der Oberfläche (wie beim Anlegen über die Fragen-Seite)
  private actor(base: BoardActor): BoardQuestionActor {
    return { ...base, kind: 'human', session: null };
  }

  // Projektschlüssel des Boards; Fragen gehören zum Projekt, nicht zum Board
  private async projectKey(boardId: number): Promise<string> {
    const boards = await this.holder.run((b) => b.services.boards.list());
    const board = boards.find((x) => x.id === boardId);
    if (!board) throw new HarnessError('board-not-found', `Board ${boardId} gibt es nicht.`);
    return board.projectKey;
  }

  // Board des Ordners; ohne Board oder Repo-Pfad lässt sich nichts anlegen
  private boardId(source: DocsSource): number {
    if (source.boardId === null) {
      throw new HarnessError('no-board', 'Zu diesem Ordner gibt es kein Board; Fragen lassen sich erst nach dem Verknüpfen anlegen.');
    }
    if (source.root === null) {
      throw new HarnessError('no-repo-path', `Das Board „${source.boardName ?? ''}“ hat keinen erreichbaren Repo-Pfad; ohne ihn gibt es keine Dokumente.`);
    }
    return source.boardId;
  }

  // Prüft Claudes Antwort: Titel Pflicht, Zeilenbereich im Dokument, höchstens MAX_QUESTION_DRAFTS
  private check(items: unknown, lines: number): { drafts: QuestionDraft[]; dropped: string[] } {
    if (!Array.isArray(items)) throw new HarnessError('analysis-failed', 'Antwort ist keine Liste von Fragen.');
    const dropped: string[] = [];
    if (items.length > MAX_QUESTION_DRAFTS) dropped.push(`Nur die ersten ${MAX_QUESTION_DRAFTS} von ${items.length} Fragen übernommen.`);
    const drafts: QuestionDraft[] = [];
    items.slice(0, MAX_QUESTION_DRAFTS).forEach((item, index) => {
      const draft = this.valid(item, lines);
      if (draft) drafts.push(draft);
      else dropped.push(`Frage ${index + 1} verworfen: Titel fehlt oder ist zu lang.`);
    });
    return { drafts, dropped };
  }

  // Ein Entwurf mit Titel (bis MAX_QUESTION_TITLE_LENGTH) und auf das Dokument begrenzter Quelle; sonst null
  private valid(raw: unknown, lines: number): QuestionDraft | null {
    if (!raw || typeof raw !== 'object') return null;
    const i = raw as Record<string, unknown>;
    const title = text(i.title);
    if (!title || title.length > MAX_QUESTION_TITLE_LENGTH) return null;
    const s = (i.source ?? {}) as Record<string, unknown>;
    const start = Number(s.startLine);
    const end = Number(s.endLine);
    const ok = Number.isInteger(start) && Number.isInteger(end) && start >= 1 && end >= start && start <= lines;
    return {
      title, description: text(i.description),
      source: { heading: text(s.heading), startLine: ok ? start : 1, endLine: ok ? Math.min(end, lines) : 1 },
    };
  }

  // Dispatcher-Einstellung für Befehl, Modell und Umgebung; ohne Einstellung die Vorgaben
  private async config(): Promise<DispatchConfig> {
    return (await new DispatchConfigStore(this.home).read()).config;
  }

  // Aufrufargumente: ein Durchgang, Antwort als JSON, keine schreibenden Werkzeuge
  private args(cfg: DispatchConfig): string[] {
    return [
      '-p', '--output-format', 'json', '--max-turns', '1',
      ...(cfg.claude.model ? ['--model', cfg.claude.model] : []),
      '--permission-mode', 'default',
      '--disallowedTools', ...DENIED_TOOLS,
    ];
  }

  // JSON-Array aus dem Antworttext schneiden; nichts Lesbares ergibt „analysis-failed“
  private items(answer: string): unknown {
    const start = answer.indexOf('[');
    const end = answer.lastIndexOf(']');
    try {
      if (start >= 0 && end > start) return JSON.parse(answer.slice(start, end + 1));
    } catch {
      /* unlesbar */
    }
    throw new HarnessError('analysis-failed', `Antwort nicht lesbar: ${excerpt(answer)}`);
  }

  // Arbeitsanweisung: offene, ungeklärte Punkte des ganzen Dokuments als eigenständige Fragen
  private prompt(content: string): string {
    return [
      'Du liest aus einem Dokument (Spec oder Implementierungsplan) die offenen, noch ungeklärten Fragen heraus.',
      '',
      'Regeln:',
      '- Eine offene Frage ist ein Punkt, den das Dokument ausdrücklich offen lässt oder der vor der Umsetzung entschieden oder recherchiert werden muss:',
      '  ein Abschnitt „Offene Fragen“, Markierungen wie TBD, TODO, „noch zu klären“, „?“, unentschiedene Alternativen oder fehlende Angaben.',
      '- Erfinde keine Fragen zu Dingen, die das Dokument schon entscheidet. Findest du keine, antworte mit [].',
      '- Der Titel ist die Frage selbst, kurz und für sich verständlich (endet mit „?“).',
      '- Die Beschreibung gibt den nötigen Kontext aus dem Dokument und nennt, falls vorhanden, die erwogenen Möglichkeiten.',
      `- Höchstens ${MAX_QUESTION_DRAFTS} Fragen; fasse Zusammengehöriges zu einer Frage zusammen.`,
      '- „source“ nennt Überschrift und Zeilenbereich der Stelle, an der die Frage steht (Zeilennummern aus dem Dokument unten).',
      '- Antworte deutsch und ausschließlich mit einem JSON-Array, ohne Markdown und ohne Werkzeuge.',
      '',
      'Jedes Element:',
      '{"title": string, "description": string, "source": {"heading": string, "startLine": number, "endLine": number}}',
      '',
      '## Dokument (mit Zeilennummern)',
      content.split(/\r?\n/).map((line, n) => `${n + 1}\t${line}`).join('\n'),
    ].join('\n');
  }
}
