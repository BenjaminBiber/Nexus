import {
  AnalysisWorkspace, ClaudeInvocation, DispatchConfigStore, HarnessError, MAX_DOC_DRAFTS, MAX_DRAFTS, TicketDraftValidator,
  type DispatchConfig, type HarnessHome, type TicketDraft,
} from '@harness/core';
import { ClaudeResultEnvelope, excerpt } from '../dispatch/ClaudeResultEnvelope.js';
import type { ClaudeRunner } from '../dispatch/SuggestionAnalyzer.js';
import { DocDraftStore } from './DocDraftStore.js';
import { join } from 'node:path';
import type { DocsListDto, DocsService } from './DocsService.js';
import type { DocsSource } from './DocsRootResolver.js';

/** Markierte Stelle im Dokument, aus der Tickets entstehen sollen */
export interface TicketDraftSelection {
  quote?: string;
  heading?: string;
  startLine?: number;
  endLine?: number;
}

export interface TicketDraftRequest {
  path?: string;
  selections?: TicketDraftSelection[];
  /** true zerlegt das ganze Dokument (ohne Markierungen) in granulare Tickets */
  whole?: boolean;
  note?: string;
}

export interface TicketDraftResult {
  drafts: TicketDraft[];
  dropped: string[];
  costUsd: number | null;
  at: string;
}

const TIMEOUT_MS = 5 * 60_000;
const MAX_QUOTE_CHARS = 2000;
const MAX_NOTE_CHARS = 2000;
/** Die Erzeugung liest nur den Prompt – keine Shell, keine Dateien, kein Web */
const DENIED_TOOLS = ['Bash', 'Edit', 'Write', 'MultiEdit', 'NotebookEdit', 'WebFetch', 'WebSearch', 'Task'];

// Erzeugt aus markierten Stellen oder einem ganzen Dokument per Claude geprüfte Ticket-Entwürfe (ohne sie anzulegen).
export class TicketDraftService {
  // Laufende Erzeugung je Projekt; über ihren AbortController lässt sie sich abbrechen
  private readonly running = new Map<string, AbortController>();
  private readonly invocation = new ClaudeInvocation();
  private readonly envelope = new ClaudeResultEnvelope();
  private readonly validator = new TicketDraftValidator();
  private readonly workspace: AnalysisWorkspace;
  private readonly store: DocDraftStore;

  constructor(
    private readonly docs: DocsService,
    private readonly claude: ClaudeRunner,
    private readonly home: HarnessHome,
    private readonly clock: () => Date = () => new Date(),
  ) {
    this.workspace = new AnalysisWorkspace(home);
    this.store = new DocDraftStore(home);
  }

  // Erzeugt die Entwürfe zu einem Dokument; je Projekt läuft höchstens eine Erzeugung
  async create(source: DocsSource, projectId: string, input: TicketDraftRequest): Promise<TicketDraftResult> {
    const whole = input.whole === true;
    const selections = (input.selections ?? []).filter((s) => s && typeof s === 'object');
    if (!whole && selections.length === 0) throw new HarnessError('invalid-input', 'Bitte mindestens eine Stelle markieren.');
    const doc = this.docs.read(source, input.path);
    if (this.running.has(projectId)) {
      throw new HarnessError('conflict', 'Für dieses Projekt läuft schon eine Erzeugung.', { reason: 'drafts-running' });
    }
    const controller = new AbortController();
    this.running.set(projectId, controller);
    try {
      const cfg = await this.config();
      const result = await this.claude.run({
        command: cfg.claude.command, args: this.args(cfg), prompt: whole ? this.wholePrompt(doc.content, input.note) : this.prompt(doc.content, selections, input.note),
        cwd: await this.workspace.dir(), env: this.invocation.env(process.env, cfg, null) as NodeJS.ProcessEnv,
        timeoutMs: TIMEOUT_MS, onOutput: () => {}, signal: controller.signal,
      });
      if (controller.signal.aborted) throw new HarnessError('drafts-cancelled', 'Die Erzeugung wurde abgebrochen.');
      if (result.code === 127) throw new HarnessError('analysis-failed', `Claude-Befehl „${cfg.claude.command}“ nicht gefunden.`);
      if (result.timedOut) throw new HarnessError('analysis-failed', 'Claude hat die Entwürfe nicht innerhalb von 5 Minuten geliefert.');
      if (result.code !== 0) throw new HarnessError('analysis-failed', `Claude endete mit Exit-Code ${result.code}: ${excerpt(result.output)}`);
      const { text, costUsd } = this.envelope.parse(result.output);
      const check = this.validator.check(this.items(text), { lines: doc.content.split(/\r?\n/).length, max: whole ? MAX_DOC_DRAFTS : MAX_DRAFTS });
      const saved = { drafts: check.drafts, dropped: check.dropped, costUsd, at: this.clock().toISOString() };
      // Gemerkt, damit sich die Entwürfe später (auch nach Neuladen oder ohne offene Seite) wieder öffnen lassen
      await this.store.put(this.docs.fullPath(source, input.path), saved);
      return saved;
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

  // Zuletzt erzeugte Entwürfe zum Dokument; null, wenn keine gespeichert sind
  async saved(source: DocsSource, path: string | undefined): Promise<TicketDraftResult | null> {
    return this.store.get(this.docs.fullPath(source, path));
  }

  // Speichert die im Dialog bearbeiteten Entwürfe (geprüft wie Claudes Antwort); Kosten und Zeitpunkt der Erzeugung bleiben
  async update(source: DocsSource, path: string | undefined, drafts: unknown): Promise<TicketDraftResult> {
    if (!Array.isArray(drafts)) throw new HarnessError('invalid-input', 'Entwürfe fehlen.');
    const doc = this.docs.read(source, path);
    const full = this.docs.fullPath(source, path);
    const before = await this.store.get(full);
    const check = this.validator.check(drafts, { lines: doc.content.split(/\r?\n/).length, max: MAX_DOC_DRAFTS });
    const saved = { drafts: check.drafts, dropped: before?.dropped ?? [], costUsd: before?.costUsd ?? null, at: before?.at ?? this.clock().toISOString() };
    await this.store.put(full, saved);
    return saved;
  }

  // Markiert in der Docs-Liste die Dateien, zu denen Entwürfe gespeichert sind (Filter „Entwürfe vorhanden“)
  async flag(list: DocsListDto): Promise<DocsListDto> {
    const root = list.source.root;
    if (!root || list.docs.length === 0) return list;
    const saved = await this.store.savedAmong(list.docs.map((d) => join(root, d.path)));
    if (saved.size === 0) return list;
    return { ...list, docs: list.docs.map((d) => (saved.has(join(root, d.path)) ? { ...d, hasDrafts: true } : d)) };
  }

  // Verwirft die gespeicherten Entwürfe zum Dokument
  async discard(source: DocsSource, path: string | undefined): Promise<void> {
    await this.store.remove(this.docs.fullPath(source, path));
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
  private items(text: string): unknown {
    const start = text.indexOf('[');
    const end = text.lastIndexOf(']');
    try {
      if (start >= 0 && end > start) return JSON.parse(text.slice(start, end + 1));
    } catch {
      /* unlesbar */
    }
    throw new HarnessError('analysis-failed', `Antwort nicht lesbar: ${excerpt(text)}`);
  }

  // Arbeitsanweisung für das ganze Dokument: granular zerlegen, bei Plänen entlang der Tasks
  private wholePrompt(content: string, note: string | undefined): string {
    return [
      'Du zerlegst ein ganzes Dokument (Spec oder Implementierungsplan) in granulare, einzeln umsetzbare Tickets.',
      '',
      'Regeln:',
      '- Decke das ganze Dokument ab: lass nichts Umsetzbares aus und erfinde nichts dazu.',
      '- Ist es ein Plan mit Tasks oder nummerierten Schritten, wird in der Regel jeder Task ein Ticket, in der Reihenfolge des Plans; seine Schritte kommen in „steps“.',
      '- Ist es eine Spec, schneide entlang der Abschnitte in kleine, für sich lieferbare Stücke.',
      '- Jedes Ticket ist für sich umsetzbar und test-first prüfbar; die Abnahme nennt, woran man das Ergebnis sieht.',
      `- Höchstens ${MAX_DOC_DRAFTS} Tickets. Reine Hintergrund- oder Begründungsabschnitte werden kein eigenes Ticket.`,
      '- „dependsOn“ sind ausschließlich 0-basierte Indizes in diese Liste; keine Kreise, kein Selbstbezug. Setze sie, wo ein Ticket auf einem früheren aufbaut.',
      '- „source“ nennt Überschrift und Zeilenbereich des Abschnitts, aus dem das Ticket stammt (Zeilennummern aus dem Dokument unten).',
      '- Antworte deutsch und ausschließlich mit einem JSON-Array, ohne Markdown und ohne Werkzeuge.',
      '',
      'Jedes Element:',
      '{"title": string, "description": string, "acceptance": string, "steps": string[], "dependsOn": number[], "labels": string[], "source": {"heading": string, "startLine": number, "endLine": number}}',
      ...(note?.trim() ? ['', '## Hinweis des Nutzers', note.trim().slice(0, MAX_NOTE_CHARS)] : []),
      '',
      '## Dokument (mit Zeilennummern)',
      this.numbered(content),
    ].join('\n');
  }

  // Dokument mit vorangestellten Zeilennummern, damit Claude die Quelle der Tickets nennen kann
  private numbered(content: string): string {
    return content.split(/\r?\n/).map((line, i) => `${i + 1}\t${line}`).join('\n');
  }

  // Arbeitsanweisung: ganzes Dokument mit Zeilennummern, die markierten Stellen, Regeln und Antwortformat
  private prompt(content: string, selections: TicketDraftSelection[], note: string | undefined): string {
    const marked = selections.map((s, i) => [
      `### Stelle ${i + 1}`,
      `Überschrift: ${(s.heading ?? '').trim() || '(keine)'}`,
      `Zeilen: ${Number(s.startLine) || 1}–${Number(s.endLine) || Number(s.startLine) || 1}`,
      (s.quote ?? '').slice(0, MAX_QUOTE_CHARS),
    ].join('\n'));
    return [
      'Du zerlegst einen Abschnitt einer Spec in einzeln umsetzbare Tickets.',
      '',
      'Regeln:',
      '- Arbeite nur zu den unten markierten Stellen, nicht zum übrigen Dokument.',
      '- Jedes Ticket ist für sich umsetzbar und test-first prüfbar; die Abnahme nennt, woran man das Ergebnis sieht.',
      `- Höchstens ${MAX_DRAFTS} Tickets. Lieber wenige gute als viele kleine Schnipsel.`,
      '- „dependsOn“ sind ausschließlich 0-basierte Indizes in diese Liste; keine Kreise, kein Selbstbezug.',
      '- „source“ nennt Überschrift und Zeilenbereich der markierten Stelle, aus der das Ticket stammt (Zeilennummern aus dem Dokument unten).',
      '- Antworte deutsch und ausschließlich mit einem JSON-Array, ohne Markdown und ohne Werkzeuge.',
      '',
      'Jedes Element:',
      '{"title": string, "description": string, "acceptance": string, "steps": string[], "dependsOn": number[], "labels": string[], "source": {"heading": string, "startLine": number, "endLine": number}}',
      '',
      '## Markierte Stellen',
      ...marked,
      ...(note?.trim() ? ['', '## Hinweis des Nutzers', note.trim().slice(0, MAX_NOTE_CHARS)] : []),
      '',
      '## Dokument (mit Zeilennummern)',
      this.numbered(content),
    ].join('\n');
  }
}
