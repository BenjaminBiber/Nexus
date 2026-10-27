import { HarnessError } from '../errors/HarnessError.js';
import { DecisionPolicy, type DecisionInput, type DecisionSubject } from '../questions/DecisionPolicy.js';
import type { Decision, QuestionState, Research } from '../questions/Question.js';
import { ResearchValidator, type ResearchInput } from '../questions/ResearchValidator.js';
import type { Actor } from '../actors/Actor.js';
import type { BoardDatabase, BoardUnitOfWork } from './BoardDatabase.js';
import { BoardLabels } from './BoardLabels.js';
import { BoardLimits } from './BoardLimits.js';
import {
  MAX_OPTION_TITLE_LENGTH, MAX_QUESTION_OCCASION_LENGTH, MAX_QUESTION_TITLE_LENGTH, MAX_SESSION_LENGTH,
  type BoardQuestion, type BoardQuestionActor, type BoardQuestionDecision, type BoardQuestionDetail, type BoardQuestionHistoryType, type BoardQuestionResearch,
  type BoardQuestionView, type NotifiableBoardQuestion,
} from './BoardQuestionTypes.js';
import { BoardQuestionViews } from './BoardQuestionViews.js';
import { BoardRepoPaths, MemoryBoardRepoStore } from './BoardRepoPaths.js';
import { QuestionConcern } from './QuestionConcern.js';
import { DependencyRules } from './DependencyRules.js';
import { ProjectService } from './ProjectService.js';
import { QuestionIds } from './QuestionIds.js';
import { TicketIds } from './TicketIds.js';

export interface NewBoardQuestionInput {
  title: string;
  description?: string;
  tags?: string[];
  occasion?: string | null;
  ticketIds?: string[];
  dependsOn?: string[];
}

// Änderbare Felder; nicht genannte bleiben
export interface BoardQuestionChanges {
  title?: string;
  description?: string;
  tags?: string[];
  occasion?: string | null;
}

// Auswahl der Liste; genannte Kriterien werden mit „und“ verknüpft
export interface BoardQuestionListFilter {
  state?: QuestionState | 'all';
  ticket?: string;
  tag?: string;
  text?: string;
}

// Frage der Liste mit Kennzeichnung „betrifft den Betrachter“ (eigene Frage, siehe QuestionConcern)
export type BoardQuestionListItem = BoardQuestionView & { mine: boolean };

// Zähler je Zustand über alle Fragen des Projekts (ohne Filter); `mine` = offene Fragen, die den Betrachter betreffen
export type BoardQuestionCounts = Record<QuestionState, number> & { mine: number };

export interface BoardQuestionList {
  questions: BoardQuestionListItem[];
  counts: BoardQuestionCounts;
}

export interface BoardQuestionWriteOptions {
  rev?: number;
}

// Ein Verlaufseintrag, bevor Zeit und Handelnder dazukommen
interface Entry {
  type: BoardQuestionHistoryType;
  text: string;
}

// Ergebnis eines Entscheidungsschritts: neuer Zustand, Entscheidung (null beim Wiedereröffnen) und Verlaufseintrag
interface DecisionStep {
  state: QuestionState;
  decision: Decision | null;
  type: BoardQuestionHistoryType;
  text: string;
}

// Höchstlänge der Notiz im Ticketverlauf (Spalte board.History.Note)
const TICKET_NOTE_LENGTH = 200;

// Fragen auf der Board-Datenbank: anlegen, lesen, auflisten, ändern, verknüpfen, kommentieren und recherchieren.
// Die Regeln für Recherchen und Entscheidungen kommen aus dem Datei-Modul (ResearchValidator, DecisionPolicy).
export class BoardQuestionService {
  private readonly ids = new QuestionIds();
  private readonly ticketIds = new TicketIds();
  private readonly deps = new DependencyRules();
  private readonly validator = new ResearchValidator();
  private readonly policy = new DecisionPolicy();
  private readonly views = new BoardQuestionViews();
  private readonly concern = new QuestionConcern();
  private readonly projects: ProjectService;

  constructor(
    private readonly db: BoardDatabase,
    private readonly clock: () => Date,
    private readonly repoPaths: BoardRepoPaths = new BoardRepoPaths(new MemoryBoardRepoStore(), null),
  ) {
    this.projects = new ProjectService(db, clock);
  }

  // Legt eine Frage im Projekt an; erst alle Prüfungen, dann die Nummer (ein Fehler rollt den Zähler zurück)
  create(projectKey: string, input: NewBoardQuestionInput, actor: BoardQuestionActor): Promise<BoardQuestionDetail> {
    return this.db.transaction(async (u) => {
      this.checkActor(actor);
      const project = await this.projects.get(u, projectKey);
      const title = this.requireTitle(input.title);
      const tags = BoardLabels.normalize(input.tags ?? []);
      const occasion = this.occasion(input.occasion);
      const ticketIds = await this.checkTickets(u, project.id, input.ticketIds ?? []);
      const number = await u.projects.takeQuestionNumber(project.id);
      const id = this.ids.format(project.key, number);
      const dependsOn = await this.checkDepends(u, id, input.dependsOn ?? []);
      const at = this.now();
      const question: BoardQuestion = {
        id, projectId: project.id, number, title, description: input.description ?? '', state: 'open', occasion,
        originKind: actor.kind, originName: actor.name, originSession: actor.session,
        tags, ticketIds, dependsOn, decision: null, createdAt: at, createdById: actor.userId, updatedAt: at, rev: 1,
      };
      await u.questions.insert(question);
      await this.history(u, question.id, actor, at, [{ type: 'created', text: 'Frage angelegt' }]);
      return this.views.detail(u, question);
    });
  }

  // Eine Frage mit Recherchen, Kommentaren und Verlauf
  get(id: string): Promise<BoardQuestionDetail> {
    return this.db.transaction(async (u) => this.views.detail(u, await this.load(u, id)));
  }

  // Fragen eines Projekts, gefiltert und sortiert (bereite zuerst, darin die ältesten), mit Zählern je Zustand.
  // Mit `viewer` (Benutzer-GUID) trägt jede Frage `mine` und counts.mine zählt die offenen eigenen; ohne ist nichts eigen.
  list(projectKey: string, filter: BoardQuestionListFilter = {}, viewer?: string): Promise<BoardQuestionList> {
    return this.db.transaction(async (u) => {
      const project = await this.projects.get(u, projectKey);
      const own = new Set(viewer ? (await u.boards.list({ ownerId: viewer })).map((b) => b.id) : []);
      const views = (await this.views.build(u, await u.questions.list({ projectId: project.id }))).map((v) => ({
        ...v,
        mine: viewer !== undefined && this.concern.concerns({ createdById: v.createdById, boardIds: v.tickets.map((t) => t.boardId) }, viewer, own),
      }));
      const counts: BoardQuestionCounts = { open: 0, answered: 0, discarded: 0, mine: 0 };
      for (const view of views) {
        counts[view.state] += 1;
        if (view.state === 'open' && view.mine) counts.mine += 1;
      }
      const ticket = filter.ticket ? this.ticketIds.canonical(filter.ticket) : undefined;
      const text = filter.text?.trim().toLowerCase() ?? '';
      const shown = views.filter((v) =>
        (!filter.state || filter.state === 'all' || v.state === filter.state) &&
        (!ticket || v.ticketIds.includes(ticket)) &&
        (!filter.tag || v.tags.includes(filter.tag)) &&
        (!text || [v.id, v.title, v.description, v.occasion ?? '', ...v.tags, ...v.ticketIds].some((p) => p.toLowerCase().includes(text))));
      return { questions: shown.sort((a, b) => this.compare(a, b)), counts };
    });
  }

  // Ändert Titel, Beschreibung, Tags und Anlass; jede tatsächliche Änderung steht im Verlauf
  update(id: string, changes: BoardQuestionChanges, actor: BoardQuestionActor, options: BoardQuestionWriteOptions = {}): Promise<BoardQuestionDetail> {
    if (Object.values(changes).every((v) => v === undefined)) {
      return Promise.reject(new HarnessError('nothing-changed', 'Keine Änderung angegeben.'));
    }
    return this.db.transaction(async (u) => {
      this.checkActor(actor);
      const before = await this.load(u, id, options.rev, true);
      const after: BoardQuestion = { ...before };
      const entries: Entry[] = [];
      if (changes.title !== undefined) {
        const title = this.requireTitle(changes.title);
        if (title !== before.title) {
          entries.push({ type: 'title', text: `${before.title} → ${title}` });
          after.title = title;
        }
      }
      if (changes.description !== undefined && changes.description !== before.description) {
        // Der Text enthält die frühere Fassung, wie im Datei-Modul
        entries.push({ type: 'description', text: before.description || '(leer)' });
        after.description = changes.description;
      }
      if (changes.tags !== undefined) {
        const tags = BoardLabels.normalize(changes.tags);
        if (tags.join('\n') !== before.tags.join('\n')) {
          entries.push({ type: 'tags', text: `${this.names(before.tags)} → ${this.names(tags)}` });
          after.tags = tags;
        }
      }
      if (changes.occasion !== undefined) {
        const occasion = this.occasion(changes.occasion);
        if (occasion !== before.occasion) {
          entries.push({ type: 'occasion', text: `${before.occasion ?? '(keiner)'} → ${occasion ?? '(keiner)'}` });
          after.occasion = occasion;
        }
      }
      if (!entries.length) return this.views.detail(u, before);
      return this.views.detail(u, await this.save(u, before, after, actor, entries));
    });
  }

  // Setzt die betroffenen Tickets (müssen existieren und zum Projekt gehören)
  setTickets(id: string, ticketIds: string[], actor: BoardQuestionActor, options: BoardQuestionWriteOptions = {}): Promise<BoardQuestionDetail> {
    return this.db.transaction(async (u) => {
      this.checkActor(actor);
      const before = await this.load(u, id, options.rev, true);
      const next = await this.checkTickets(u, before.projectId, ticketIds);
      if (next.join('\n') === before.ticketIds.join('\n')) return this.views.detail(u, before);
      const text = `${this.names(before.ticketIds)} → ${this.names(next)}`;
      return this.views.detail(u, await this.save(u, before, { ...before, ticketIds: next }, actor, [{ type: 'tickets', text }]));
    });
  }

  // Setzt die Abhängigkeiten (Fragen oder Tickets); Unbekanntes, Selbstbezug und Kreise werden abgelehnt
  setDepends(id: string, dependsOn: string[], actor: BoardQuestionActor, options: BoardQuestionWriteOptions = {}): Promise<BoardQuestionDetail> {
    return this.db.transaction(async (u) => {
      this.checkActor(actor);
      const before = await this.load(u, id, options.rev, true);
      const next = await this.checkDepends(u, before.id, dependsOn);
      if (next.join('\n') === before.dependsOn.join('\n')) return this.views.detail(u, before);
      const text = `${this.names(before.dependsOn)} → ${this.names(next)}`;
      return this.views.detail(u, await this.save(u, before, { ...before, dependsOn: next }, actor, [{ type: 'depends', text }]));
    });
  }

  // Hängt einen Kommentar an; er erhöht die Version der Frage
  comment(id: string, body: string, actor: BoardQuestionActor): Promise<BoardQuestionDetail> {
    const text = body.trim();
    if (!text) return Promise.reject(new HarnessError('comment-empty', 'Der Kommentar darf nicht leer sein.'));
    return this.db.transaction(async (u) => {
      this.checkActor(actor);
      const before = await this.load(u, id, undefined, true);
      const at = this.now();
      await u.questionComments.insert({ questionId: before.id, authorId: actor.userId, authorName: actor.name, authorKind: actor.kind, body: text, createdAt: at });
      return this.views.detail(u, await this.save(u, before, before, actor, [], at));
    });
  }

  // Ergänzt eine Recherche (nur anhängend); Optionen zählen über alle Recherchen der Frage weiter
  addResearch(id: string, input: ResearchInput, actor: BoardQuestionActor): Promise<BoardQuestionDetail> {
    return this.db.transaction(async (u) => {
      this.checkActor(actor);
      const before = await this.load(u, id, undefined, true);
      const at = this.now();
      const existing = (await u.questionResearch.list(before.id)).map((r) => this.toResearch(r));
      const built = this.validator.build(input, existing, this.fileActor(actor), at);
      for (const option of built.options) BoardLimits.text(option.title, MAX_OPTION_TITLE_LENGTH, 'Der Titel einer Option');
      await u.questionResearch.insert({
        questionId: before.id, at, byId: actor.userId, byName: actor.name, kind: actor.kind, context: built.context, options: built.options,
      });
      const entry: Entry = { type: 'research', text: this.validator.describe(built) };
      return this.views.detail(u, await this.save(u, before, before, actor, [entry], at));
    });
  }

  // Beantwortet eine offene Frage (nur Mensch): Option der neuesten Recherche oder Freitext, immer mit Begründung
  answer(id: string, input: DecisionInput, actor: BoardQuestionActor, options: BoardQuestionWriteOptions = {}): Promise<BoardQuestionDetail> {
    return this.decide(id, actor, options, 'Frage entschieden', (subject, at) => {
      const decision = this.policy.answer(subject, input, this.fileActor(actor), at);
      return { state: 'answered', decision, type: 'decided', text: this.policy.describe(subject, decision) };
    });
  }

  // Verwirft eine offene Frage (nur Mensch) mit Begründung
  discard(id: string, reason: string | undefined, actor: BoardQuestionActor, options: BoardQuestionWriteOptions = {}): Promise<BoardQuestionDetail> {
    return this.decide(id, actor, options, 'Frage verworfen', (subject, at) => {
      const decision = this.policy.discard(subject, reason, this.fileActor(actor), at);
      return { state: 'discarded', decision, type: 'discarded', text: this.policy.describe(subject, decision) };
    });
  }

  // Öffnet eine entschiedene oder verworfene Frage wieder (nur Mensch); die frühere Entscheidung bleibt im Verlauf
  reopen(id: string, reason: string | undefined, actor: BoardQuestionActor, options: BoardQuestionWriteOptions = {}): Promise<BoardQuestionDetail> {
    return this.decide(id, actor, options, 'Frage wieder geöffnet', (subject, _at, before) => {
      this.policy.assertReopenable(subject, this.fileActor(actor));
      const earlier = before.decision
        ? `Frühere Entscheidung: ${this.policy.describe(subject, this.fileDecision(before.decision))} (${before.decision.byName})`
        : '';
      const text = ['Wieder geöffnet.', reason?.trim() ? `Grund: ${reason.trim()}.` : '', earlier].filter(Boolean).join(' ');
      return { state: 'open', decision: null, type: 'reopened', text };
    });
  }

  // Offene Fragen, die einen Benutzer betreffen („Eigene“: selbst gestellt oder zu Tickets auf seinen Boards), mit ihren
  // Recherchen (für Windows-Meldungen)
  notifiable(ownerId: string): Promise<NotifiableBoardQuestion[]> {
    return this.db.transaction(async (u) => {
      const boards = await this.repoPaths.apply(await u.boards.list({ ownerId }));
      const own = new Set(boards.map((b) => b.id));
      const tickets = boards.length ? await u.tickets.list({ boardIds: boards.map((b) => b.id) }) : [];
      const boardOf = new Map(tickets.map((t) => [t.id, t.boardId]));
      const repoOf = new Map(boards.map((b) => [b.id, b.repoPath]));
      const open = (await u.questions.list({ states: ['open'] }))
        .filter((q) => this.concern.concerns({ createdById: q.createdById, boardIds: q.ticketIds.flatMap((t) => boardOf.get(t) ?? []) }, ownerId, own));
      const projects = new Map((await u.projects.list()).map((p) => [p.id, p]));
      const result: NotifiableBoardQuestion[] = [];
      for (const q of open) {
        const research = await u.questionResearch.list(q.id);
        // Repo-Pfade der eigenen Boards mit betroffenen Tickets, sonst der eigenen Boards im Projekt der Frage
        const byTicket = q.ticketIds.map((t) => repoOf.get(boardOf.get(t) ?? -1) ?? null);
        const byProject = boards.filter((b) => b.projectId === q.projectId).sort((a, b) => a.id - b.id).map((b) => b.repoPath);
        const found = byTicket.some(Boolean) ? byTicket : byProject;
        const repoPaths = [...new Set(found.filter((p): p is string => !!p))];
        result.push({
          id: q.id, title: q.title, occasion: q.occasion, origin: { kind: q.originKind },
          research: research.map((r) => ({ kind: r.kind, options: r.options })),
          projectKey: projects.get(q.projectId)?.key ?? '?', projectName: projects.get(q.projectId)?.name ?? '?', repoPaths,
        });
      }
      return result;
    });
  }

  // Gemeinsamer Ablauf: Frage gesperrt lesen (mit Version), Regel anwenden, speichern, Verlauf an Frage und Tickets
  private decide(
    id: string,
    actor: BoardQuestionActor,
    options: BoardQuestionWriteOptions,
    label: string,
    step: (subject: DecisionSubject, at: string, before: BoardQuestion) => DecisionStep,
  ): Promise<BoardQuestionDetail> {
    return this.db.transaction(async (u) => {
      this.checkActor(actor);
      const before = await this.load(u, id, options.rev, true);
      const research = (await u.questionResearch.list(before.id)).map((r) => this.toResearch(r));
      const at = this.now();
      const result = step({ id: before.id, state: before.state, research }, at, before);
      const after: BoardQuestion = { ...before, state: result.state, decision: result.decision ? this.boardDecision(result.decision, actor) : null };
      const saved = await this.save(u, before, after, actor, [{ type: result.type, text: result.text }], at);
      await this.noteTickets(u, before, saved, label, actor, at);
      return this.views.detail(u, saved);
    });
  }

  // Verlaufseintrag an jedem betroffenen Ticket: Feld „question“, Zustand vorher → nachher
  private async noteTickets(u: BoardUnitOfWork, before: BoardQuestion, after: BoardQuestion, label: string, actor: BoardQuestionActor, at: string): Promise<void> {
    const note = this.clip(`${label}: ${after.id} „${after.title}“`, TICKET_NOTE_LENGTH);
    const entries = after.ticketIds.map((ticketId) => ({
      ticketId, at, byId: actor.userId, byName: actor.name, field: 'question', from: before.state, to: after.state, note,
    }));
    if (entries.length) await u.history.insert(entries);
  }

  // Entscheidung des Datei-Moduls als Entscheidung der Board-Datenbank (mit Benutzer-GUID)
  private boardDecision(d: Decision, actor: BoardQuestionActor): BoardQuestionDecision {
    return { outcome: d.outcome, option: d.option, text: d.text, reason: d.reason, byId: actor.userId, byName: d.by, at: d.at };
  }

  // Entscheidung der Board-Datenbank im Format des Datei-Moduls (für DecisionPolicy.describe)
  private fileDecision(d: BoardQuestionDecision): Decision {
    return { outcome: d.outcome, option: d.option, text: d.text, reason: d.reason, by: d.byName, at: d.at };
  }

  // Kürzt einen Text auf höchstens max Zeichen (mit „…“)
  private clip(text: string, max: number): string {
    return text.length > max ? `${text.slice(0, max - 1)}…` : text;
  }

  // Liest eine Frage (Schreibweise egal) und prüft die erwartete Version
  async load(u: BoardUnitOfWork, id: string, rev?: number, forUpdate = false): Promise<BoardQuestion> {
    const canonical = this.ids.canonical(id);
    const question = await u.questions.byId(canonical, forUpdate);
    if (!question) throw new HarnessError('question-not-found', `Frage ${canonical} gibt es nicht.`);
    if (rev !== undefined && rev !== question.rev) {
      throw new HarnessError('conflict', `Frage ${canonical} wurde inzwischen geändert (Version ${question.rev}, erwartet ${rev}). Bitte neu lesen.`);
    }
    return question;
  }

  // Schreibt die neue Fassung mit nächster Version und Zeitstempel und protokolliert die Einträge
  async save(u: BoardUnitOfWork, before: BoardQuestion, after: BoardQuestion, actor: BoardQuestionActor, entries: Entry[], at = this.now()): Promise<BoardQuestion> {
    const next: BoardQuestion = { ...after, rev: before.rev + 1, updatedAt: at };
    await u.questions.update(next);
    await this.history(u, next.id, actor, at, entries);
    return next;
  }

  // Schreibt Verlaufseinträge mit Zeit, Handelndem und Art
  private async history(u: BoardUnitOfWork, questionId: string, actor: BoardQuestionActor, at: string, entries: Entry[]): Promise<void> {
    if (!entries.length) return;
    await u.questionHistory.insert(entries.map((e) => ({
      questionId, at, byId: actor.userId, byName: actor.name, byKind: actor.kind, type: e.type, text: e.text,
    })));
  }

  // Betroffene Tickets: Schreibweise vereinheitlichen, Doppelte entfernen, sortieren; jedes muss im Projekt existieren
  private async checkTickets(u: BoardUnitOfWork, projectId: number, raw: string[]): Promise<string[]> {
    const ids = [...new Set(raw.filter((r) => r.trim()).map((r) => this.ticketIds.canonical(r)))].sort();
    if (!ids.length) return [];
    const found = new Map((await u.tickets.list({ ids })).map((t) => [t.id, t]));
    for (const id of ids) {
      const ticket = found.get(id);
      if (!ticket) throw new HarnessError('question-ticket-unknown', `Ticket ${id} gibt es nicht.`);
      if (ticket.projectId !== projectId) {
        throw new HarnessError('question-ticket-other-project', `Ticket ${id} gehört zu einem anderen Projekt. Eine Frage betrifft nur Tickets ihres Projekts.`);
      }
    }
    return ids;
  }

  // Abhängigkeiten: Verweise vereinheitlichen (Reihenfolge bleibt), dann gegen alle Fragen und die genannten Tickets prüfen.
  // Tickets hängen nie von Fragen ab, deshalb sind sie im Graphen Blätter.
  private async checkDepends(u: BoardUnitOfWork, id: string, raw: string[]): Promise<string[]> {
    const refs: string[] = [];
    for (const item of raw) {
      if (!item.trim()) continue;
      const ref = this.ids.reference(item);
      if (!refs.includes(ref)) refs.push(ref);
    }
    if (!refs.length) return [];
    const graph = new Map<string, string[]>((await u.questions.list()).map((q) => [q.id, q.dependsOn.filter((d) => this.ids.isQuestion(d))]));
    const tickets = refs.filter((r) => !this.ids.isQuestion(r));
    if (tickets.length) for (const t of await u.tickets.list({ ids: tickets })) graph.set(t.id, []);
    this.deps.validate(id, refs, graph);
    return refs;
  }

  // Name des Handelnden und Kennung der Session dürfen die Spalten nicht sprengen
  private checkActor(actor: BoardQuestionActor): void {
    BoardLimits.actor(actor);
    BoardLimits.text(actor.session, MAX_SESSION_LENGTH, 'Die Kennung der Session');
  }

  // Titel ohne Leerraum am Rand; leer oder zu lang ist nicht erlaubt
  private requireTitle(title: string): string {
    const value = title.trim();
    if (!value) throw new HarnessError('title-empty', 'Der Titel darf nicht leer sein.');
    return BoardLimits.text(value, MAX_QUESTION_TITLE_LENGTH, 'Der Titel');
  }

  // Anlass ohne Leerraum am Rand; leer wird zu null
  private occasion(value: string | null | undefined): string | null {
    const text = value && value.trim() ? value.trim() : null;
    return BoardLimits.text(text, MAX_QUESTION_OCCASION_LENGTH, 'Der Anlass');
  }

  // Bereite zuerst, darin die ältesten zuerst, bei Gleichstand nach Nummer (wie QuestionFilter im Datei-Modul)
  private compare(a: BoardQuestionView, b: BoardQuestionView): number {
    return Number(b.ready) - Number(a.ready) || a.createdAt.localeCompare(b.createdAt) || a.projectId - b.projectId || a.number - b.number;
  }

  // Recherche im Format des Datei-Moduls (für ResearchValidator und DecisionPolicy)
  protected toResearch(r: BoardQuestionResearch): Research {
    return { at: r.at, author: r.byName, kind: r.kind, context: r.context, options: r.options };
  }

  // Handelnder im Format des Datei-Moduls
  protected fileActor(actor: BoardQuestionActor): Actor {
    return { author: actor.name, kind: actor.kind };
  }

  // Liste als Text, leer als „(keine)“
  private names(items: string[]): string {
    return items.length ? items.join(', ') : '(keine)';
  }

  // Aktuelle Zeit als ISO-Text
  private now(): string {
    return this.clock().toISOString();
  }
}
