import { HarnessError } from '../errors/HarnessError.js';
import type { BoardDatabase, BoardUnitOfWork } from './BoardDatabase.js';
import {
  HUMAN_INTERACTION_PREFIX, MAX_BLOCKED_REASON_LENGTH, MAX_BRANCH_LENGTH, MAX_PERSON_NAME_LENGTH, MAX_SESSION_ID_LENGTH, MAX_SPEC_LENGTH, MAX_SPEC_TASK_LENGTH,
  MAX_STEP_TEXT_LENGTH, MAX_TITLE_LENGTH, MAX_WORKTREE_LENGTH, PRIORITIES,
  isHumanInteraction, type BoardActor, type Comment, type HistoryEntry, type Priority, type Ticket, type TicketStatus, type TicketStep,
} from './BoardTypes.js';
import { BoardLabels } from './BoardLabels.js';
import { BoardLimits } from './BoardLimits.js';
import { BoardQuestionBlocking } from './BoardQuestionBlocking.js';
import type { BoardOpenQuestion } from './BoardQuestionTypes.js';
import { BoardService } from './BoardService.js';
import { DependencyCleanup } from './DependencyCleanup.js';
import { DependencyGraph } from './DependencyGraph.js';
import { QuestionTicketCleanup } from './QuestionTicketCleanup.js';
import { DependencyRules } from './DependencyRules.js';
import { TargetBranchRule } from '../git/TargetBranchRule.js';
import { TicketHistoryDiff } from './TicketHistoryDiff.js';
import { TicketIds } from './TicketIds.js';
import { TicketOrdering, type BoardColumn } from './TicketOrdering.js';
import { TicketStepEdits, type StepChange, type StepEdit, type StepGuard } from './TicketStepEdits.js';

export interface NewTicketInput {
  title: string;
  description?: string;
  status?: TicketStatus;
  priority?: Priority;
  labels?: string[];
  dependsOn?: string[];
  acceptance?: string;
  needsPlanning?: boolean;
  spec?: string | null;
  specTask?: string | null;
  targetBranch?: string | null;
  touches?: string[];
  consumes?: string[];
  produces?: string[];
  steps?: TicketStep[];
}

export interface TicketPatch {
  title?: string;
  description?: string;
  plan?: string;
  handbook?: string;
  diagram?: string;
  status?: TicketStatus;
  priority?: Priority;
  labels?: string[];
  assignee?: string | null;
  needsPlanning?: boolean;
  blockedReason?: string | null;
  acceptance?: string;
  dependsOn?: string[];
  dependsOnAdd?: string[];
  dependsOnRemove?: string[];
  boardId?: number;
  spec?: string | null;
  specTask?: string | null;
  branch?: string | null;
  worktree?: string | null;
  targetBranch?: string | null;
  touches?: string[];
  consumes?: string[];
  produces?: string[];
  steps?: TicketStep[];
}

// force: Abhängigkeits- und not_needed-Sperre übergehen; allowDuplicate: beim Anlegen ein offenes Ticket mit gleichem
// Titel zulassen; byUser: Aktion eines Menschen in der Oberfläche
export interface WriteOptions {
  rev?: number;
  force?: boolean;
  allowDuplicate?: boolean;
  byUser?: boolean;
}

export interface TicketView extends Ticket {
  column: BoardColumn;
  waiting: boolean;
  dependsOpen: string[];
  // Erledigte (oder nicht mehr nötige) Voraussetzungen mit noch gesetztem Branch: noch nicht gemergt
  dependsUnmerged: string[];
  dependents: string[];
  // Abhängige, deren letzte offene Voraussetzung dieses Ticket ist (werden mit seinem Erledigt bereit)
  unlocks: string[];
  // Offene Fragen, die dieses Ticket betreffen (sperren In Arbeit, Erledigt, claim und next-planning)
  waitingOnQuestions: BoardOpenQuestion[];
  // Erledigte und alle Arbeitsschritte
  stepsDone: number;
  stepsTotal: number;
  // Verknüpfte Claude-Sessions in Reihenfolge der Verknüpfung
  sessions: string[];
}

export interface TicketDetail extends TicketView {
  comments: Comment[];
  history: HistoryEntry[];
  steps: TicketStep[];
}

// Kurzangaben eines Tickets für Sessions, Favoriten und Meldungen
export interface TicketSummary {
  id: string;
  title: string;
  status: TicketStatus;
  boardId: number;
}

export interface TicketListFilter {
  boardIds: number[];
  columns?: BoardColumn[];
  needsPlanning?: boolean;
  assignee?: string;
  label?: string;
  q?: string;
  // Lange Texte nur als Kennzeichen (TicketFilter.withoutTexts); mit q wird trotzdem voll geladen (Suche in der Beschreibung)
  withoutTexts?: boolean;
}

// Tickets anlegen, lesen, ändern, verschieben, löschen, übernehmen und freigeben (Regeln des alten Boards).
export class TicketService {
  private readonly boards: BoardService;
  private readonly ids = new TicketIds();
  private readonly deps = new DependencyRules();
  private readonly depGraph = new DependencyGraph();
  private readonly diff = new TicketHistoryDiff();
  private readonly ordering = new TicketOrdering();
  private readonly cleanup = new DependencyCleanup();
  private readonly blocking = new BoardQuestionBlocking();
  private readonly questionCleanup = new QuestionTicketCleanup();
  private readonly stepEdits = new TicketStepEdits();
  private readonly targetRule = new TargetBranchRule();

  constructor(private readonly db: BoardDatabase, private readonly clock: () => Date) {
    this.boards = new BoardService(db, clock);
  }

  // Legt ein Ticket auf dem Board an; ein offenes Ticket mit gleichem Titel auf dem Board lehnt es ab (allowDuplicate übergeht das)
  create(boardId: number, input: NewTicketInput, actor: BoardActor, options: WriteOptions = {}): Promise<TicketView> {
    return this.db.transaction(async (u) => {
      if (!options.allowDuplicate) await this.rejectDuplicate(u, boardId, input.title);
      return this.view(u, await this.createIn(u, boardId, input, actor, options));
    });
  }

  // Ein Ticket mit Kommentaren, Verlauf und Schritten
  get(id: string): Promise<TicketDetail> {
    return this.db.transaction(async (u) => this.detail(u, (await this.load(u, id)).id));
  }

  // Tickets der Boards, gefiltert und in Anzeige-Reihenfolge
  list(filter: TicketListFilter): Promise<TicketView[]> {
    return this.db.transaction(async (u) => {
      const q = filter.q?.toLowerCase();
      const tickets = await u.tickets.list({ boardIds: filter.boardIds, withoutTexts: !!filter.withoutTexts && !q });
      const views = (await this.views(u, tickets)).filter((t) =>
        (!filter.columns || filter.columns.includes(t.column)) &&
        (filter.needsPlanning === undefined || t.needsPlanning === filter.needsPlanning) &&
        (filter.assignee === undefined || t.assignee === filter.assignee) &&
        (filter.label === undefined || t.labels.includes(filter.label)) &&
        (!q || `${t.id} ${t.title} ${t.description}`.toLowerCase().includes(q)));
      return this.sortViews(views);
    });
  }

  // Sortiert Anzeigefassungen in Anzeige-Reihenfolge (TicketOrdering gibt dieselben Objekte zurück)
  sortViews(views: TicketView[]): TicketView[] {
    const waiting = new Set(views.filter((v) => v.waiting).map((v) => v.id));
    return this.ordering.sort(views, waiting) as TicketView[];
  }

  // Ändert Felder; Statuswechsel unterliegen denselben Sperren wie move. Schritte werden als Ganzes ersetzt.
  update(id: string, patch: TicketPatch, actor: BoardActor, options: WriteOptions = {}): Promise<TicketView> {
    if (Object.values(patch).every((v) => v === undefined)) {
      return Promise.reject(new HarnessError('nothing-changed', 'Keine Änderung angegeben.'));
    }
    return this.db.transaction(async (u) => {
      const before = await this.load(u, id, options.rev, true);
      const after = await this.apply(u, before, patch);
      const steps = patch.steps === undefined ? undefined : this.cleanSteps(patch.steps);
      await this.checkStatus(u, before, after, options);
      const saved = await this.save(u, before, after, actor, null);
      if (steps) await this.writeSteps(u, saved, steps, actor);
      return this.view(u, saved);
    });
  }

  // Verschiebt in eine Spalte; außerhalb von Blockiert wird der Grund gelöscht
  move(id: string, status: TicketStatus, actor: BoardActor, options: WriteOptions & { acceptance?: string } = {}): Promise<TicketView> {
    return this.db.transaction(async (u) => {
      const before = await this.load(u, id, options.rev, true);
      const after: Ticket = {
        ...before,
        status,
        acceptance: options.acceptance ?? before.acceptance,
        blockedReason: status === 'blocked' ? before.blockedReason : null,
      };
      if (status !== before.status) after.sortOrder = await this.nextOrder(u, before.boardId, status);
      if (this.diff.diff(before, after, actor, '', null).length === 0) return this.view(u, before);
      await this.checkStatus(u, before, after, options);
      return this.view(u, await this.save(u, before, after, actor, 'verschoben'));
    });
  }

  // Abnahme durch einen Menschen: nur Blockiert mit Grund „Human Interaction: …“; setzt Erledigt (Abnahme-Text Pflicht wie bei done),
  // leert den Grund und vermerkt „abgenommen“ im Verlauf
  accept(id: string, actor: BoardActor, options: WriteOptions = {}): Promise<TicketView> {
    return this.db.transaction(async (u) => {
      const before = await this.load(u, id, options.rev, true);
      if (before.status !== 'blocked' || !isHumanInteraction(before.blockedReason)) {
        throw new HarnessError('accept-not-possible', `${before.id} wartet auf keine Abnahme (nur Blockiert mit Grund „${HUMAN_INTERACTION_PREFIX} …“).`);
      }
      const after: Ticket = { ...before, status: 'done', blockedReason: null, sortOrder: await this.nextOrder(u, before.boardId, 'done') };
      await this.checkStatus(u, before, after, options);
      return this.view(u, await this.save(u, before, after, actor, 'abgenommen'));
    });
  }

  // Holt Tickets von anderen Boards desselben Projekts auf das Ziel-Board (eine Transaktion, alles oder nichts).
  // Jedes Ticket kommt ans Ende seiner Spalte; der Verlauf nennt das Quell-Board.
  moveToBoard(ids: string[], targetBoardId: number, actor: BoardActor, _options: WriteOptions = {}): Promise<TicketView[]> {
    if (ids.length === 0) return Promise.reject(new HarnessError('nothing-changed', 'Keine Tickets angegeben.'));
    return this.db.transaction(async (u) => {
      BoardLimits.actor(actor);
      const target = await this.boards.get(u, targetBoardId);
      const wanted = [...new Set(ids.map((id) => this.ids.canonical(id)))];
      const tickets: Ticket[] = [];
      for (const id of wanted) tickets.push(await this.load(u, id, undefined, true));
      for (const ticket of tickets) {
        if (ticket.boardId === target.id) {
          throw new HarnessError('ticket-already-on-board', `${ticket.id} liegt schon auf Board ${target.name}.`);
        }
        if (ticket.projectId !== target.projectId) {
          throw new HarnessError('board-other-project', `${ticket.id} gehört zu einem anderen Projekt. Tickets wechseln nur zwischen Boards desselben Projekts.`);
        }
      }
      const sourceNames = new Map<number, string>();
      const moved: Ticket[] = [];
      for (const before of tickets) {
        if (!sourceNames.has(before.boardId)) sourceNames.set(before.boardId, (await this.boards.get(u, before.boardId)).name);
        const after: Ticket = { ...before, boardId: target.id, sortOrder: await this.nextOrder(u, target.id, before.status) };
        moved.push(await this.save(u, before, after, actor, `von Board ${sourceNames.get(before.boardId)} geholt`));
      }
      return this.views(u, moved);
    });
  }

  // Löscht endgültig, entfernt Verweise anderer Tickets und protokolliert die Löschung
  delete(id: string, actor: BoardActor): Promise<void> {
    return this.db.transaction(async (u) => {
      BoardLimits.actor(actor);
      const ticket = await this.load(u, id, undefined, true);
      const at = this.now();
      await this.cleanup.remove(u, ticket.id, actor, at);
      await this.questionCleanup.remove(u, ticket, actor, at);
      await u.tickets.delete(ticket.id);
      await u.deletions.insert({ at, byId: actor.userId, byName: actor.name, ticketId: ticket.id, ticketTitle: ticket.title, fromStatus: ticket.status });
    });
  }

  // Übernimmt ein Ticket für einen Agenten (Standard: In Arbeit); bereits zugewiesen (auch an denselben
  // Namen) oder mit offener Voraussetzung nur mit force. Die Abnahme wird hier nicht geprüft. Mit rev nur,
  // wenn das Ticket seitdem unverändert ist (sonst conflict).
  claim(
    id: string, agent: string, actor: BoardActor, options: { status?: TicketStatus; force?: boolean; sessionId?: string; rev?: number } = {},
  ): Promise<TicketView> {
    const name = agent.trim();
    if (!name) return Promise.reject(new HarnessError('agent-missing', 'Für die Übernahme fehlt der Name des Agenten (--agent).'));
    return this.db.transaction(async (u) => {
      BoardLimits.text(name, MAX_PERSON_NAME_LENGTH, 'Der Name des Agenten');
      const before = await this.load(u, id, options.rev, true);
      // Auch bei gleichem Namen: mehrere Sitzungen heißen oft gleich, nur so bleibt die Übernahme exklusiv
      if (before.assignee && !options.force) {
        throw new HarnessError('conflict', `${before.id} ist bereits ${before.assignee} zugewiesen. Ist es deins, einfach weitermachen; sonst nächstes freies Ticket nehmen oder nachfragen.`);
      }
      const status = options.status ?? 'in_progress';
      if (status === 'not_needed' && !options.force) {
        throw new HarnessError('not-needed-user', '„Nicht mehr nötig“ entscheidet der Nutzer.');
      }
      if (!options.force) await this.requireNoOpenDependency(u, before);
      const after: Ticket = { ...before, assignee: name, status };
      if (status !== before.status) after.sortOrder = await this.nextOrder(u, before.boardId, status);
      const saved = await this.save(u, before, after, actor, 'übernommen');
      if (options.sessionId) await this.addSession(u, saved.id, options.sessionId);
      return this.view(u, saved);
    });
  }

  // Gibt ein Ticket frei; der Status ändert sich nur, wenn er angegeben ist (ohne Sperren)
  release(id: string, actor: BoardActor, options: { status?: TicketStatus } = {}): Promise<TicketView> {
    return this.db.transaction(async (u) => {
      const before = await this.load(u, id, undefined, true);
      const after: Ticket = { ...before, assignee: null, status: options.status ?? before.status };
      if (after.status !== before.status) after.sortOrder = await this.nextOrder(u, before.boardId, after.status);
      return this.view(u, await this.save(u, before, after, actor, 'freigegeben'));
    });
  }

  // Hakt einen Schritt ab (Nummer ab 1) oder nimmt den Haken zurück; erhöht die Version wie ein Kommentar.
  // Mit guard.rev bzw. guard.expectText nur, wenn Ticket bzw. Schritt seitdem unverändert sind (sonst conflict).
  setStep(id: string, number: number, done: boolean, actor: BoardActor, guard: StepGuard = {}): Promise<TicketDetail> {
    return this.db.transaction(async (u) => {
      BoardLimits.actor(actor);
      const before = await this.load(u, id, guard.rev, true);
      const steps = await u.steps.list(before.id);
      this.stepEdits.expect(before.id, steps, number, guard.expectText);
      const step = steps[this.stepEdits.index(before.id, steps, number)];
      if (step.done !== done) {
        const at = this.now();
        await u.steps.setDone(before.id, number - 1, done);
        await u.tickets.update({ ...before, rev: before.rev + 1, updatedAt: at });
        await u.history.insert([{
          ticketId: before.id, at, byId: actor.userId, byName: actor.name, field: 'step',
          from: `${number}: ${step.done ? 'erledigt' : 'offen'}`, to: `${number}: ${done ? 'erledigt' : 'offen'}`, note: step.text.slice(0, 200),
        }]);
      }
      return this.detail(u, before.id);
    });
  }

  // Hängt einen Schritt an (oder fügt ihn an Position at ab 1 ein); erhöht die Version und schreibt den Verlauf.
  // expectText prüft den Schritt, der bisher an Position at steht (nur mit at); rev die Ticket-Version.
  addStep(
    id: string, text: string, actor: BoardActor, options: { detail?: string | null; at?: number } & StepGuard = {},
  ): Promise<TicketDetail> {
    const { rev, expectText, ...position } = options;
    return this.changeSteps(id, actor, { rev }, (ticketId, steps) => {
      if (position.at !== undefined) this.stepEdits.expect(ticketId, steps, position.at, expectText);
      return this.stepEdits.add(ticketId, steps, text, position);
    });
  }

  // Ändert Text und/oder Anleitung eines Schritts (Nummer ab 1); der Haken bleibt
  editStep(id: string, number: number, change: StepChange, actor: BoardActor, guard: StepGuard = {}): Promise<TicketDetail> {
    return this.changeSteps(id, actor, guard, (ticketId, steps) => {
      this.stepEdits.expect(ticketId, steps, number, guard.expectText);
      return this.stepEdits.edit(ticketId, steps, number, change);
    });
  }

  // Löscht einen Schritt (Nummer ab 1); die folgenden rücken nach
  removeStep(id: string, number: number, actor: BoardActor, guard: StepGuard = {}): Promise<TicketDetail> {
    return this.changeSteps(id, actor, guard, (ticketId, steps) => {
      this.stepEdits.expect(ticketId, steps, number, guard.expectText);
      return this.stepEdits.remove(ticketId, steps, number);
    });
  }

  // Verschiebt einen Schritt von Nummer from auf Nummer to; expectText prüft den Schritt an Nummer from
  moveStep(id: string, from: number, to: number, actor: BoardActor, guard: StepGuard = {}): Promise<TicketDetail> {
    return this.changeSteps(id, actor, guard, (ticketId, steps) => {
      this.stepEdits.expect(ticketId, steps, from, guard.expectText);
      return this.stepEdits.move(ticketId, steps, from, to);
    });
  }

  // Verknüpft eine Claude-Session mit dem Ticket (ohne neue Version); false, wenn sie schon verknüpft war
  linkSession(id: string, sessionId: string): Promise<boolean> {
    return this.db.transaction(async (u) => this.addSession(u, (await this.load(u, id)).id, sessionId));
  }

  // Prüft, ob das Ticket nach In Arbeit dürfte (für harness board start, bevor ein Worktree entsteht)
  checkStart(id: string, options: { force?: boolean } = {}): Promise<TicketView> {
    return this.db.transaction(async (u) => {
      const ticket = await this.load(u, id);
      if (ticket.status !== 'in_progress') await this.guardStatus(u, { ...ticket, status: 'in_progress' }, { force: options.force });
      return this.view(u, ticket);
    });
  }

  // Kurzangaben zu Tickets in der Reihenfolge der Anfrage; unbekannte und ungültige IDs fehlen
  summaries(ids: string[]): Promise<TicketSummary[]> {
    const wanted = ids.flatMap((id) => {
      try {
        return [this.ids.canonical(id)];
      } catch {
        return [];
      }
    });
    if (!wanted.length) return Promise.resolve([]);
    return this.db.transaction(async (u) => {
      const found = new Map((await u.tickets.list({ ids: [...new Set(wanted)] })).map((t) => [t.id, t]));
      return wanted.flatMap((id) => {
        const t = found.get(id);
        return t ? [{ id: t.id, title: t.title, status: t.status, boardId: t.boardId }] : [];
      });
    });
  }

  // Tickets, mit denen eine Session verknüpft ist (sortiert)
  ticketsOfSession(sessionId: string): Promise<string[]> {
    return this.db.transaction((u) => u.ticketSessions.ticketsOf(sessionId));
  }

  // Legt ein Ticket innerhalb einer laufenden Transaktion an (auch für Notizen)
  async createIn(u: BoardUnitOfWork, boardId: number, input: NewTicketInput, actor: BoardActor, options: WriteOptions = {}): Promise<Ticket> {
    BoardLimits.actor(actor);
    const board = await this.boards.get(u, boardId);
    const project = (await u.projects.byId(board.projectId))!;
    const title = this.requireTitle(input.title);
    const priority = this.requirePriority(input.priority ?? 'normal');
    const status = input.status ?? 'backlog';
    const dependsOn = this.deps.normalize(input.dependsOn ?? []);
    const steps = this.cleanSteps(input.steps ?? []);
    const number = await u.projects.takeTicketNumber(project.id);
    const id = this.ids.format(project.key, number);
    if (dependsOn.length) this.deps.validate(id, dependsOn, await this.graph(u));
    const at = this.now();
    const ticket: Ticket = {
      id, projectId: project.id, boardId, number, title,
      description: input.description ?? '', status, priority,
      labels: BoardLabels.normalize(input.labels ?? []), dependsOn,
      sortOrder: await this.nextOrder(u, boardId, status),
      assignee: null, needsPlanning: input.needsPlanning ?? true, acceptance: input.acceptance ?? '',
      plan: '', handbook: '', diagram: '', blockedReason: null,
      spec: this.optional(input.spec, MAX_SPEC_LENGTH, 'Die Spec'),
      specTask: this.optional(input.specTask, MAX_SPEC_TASK_LENGTH, 'Die Spec-Aufgabe'),
      branch: null, worktree: null, targetBranch: this.targetRule.normalize(input.targetBranch),
      touches: this.cleanList(input.touches ?? []), consumes: this.cleanList(input.consumes ?? []), produces: this.cleanList(input.produces ?? []),
      plannedAt: null, plannedBy: null, planningClaimBy: null, planningClaimAt: null,
      createdAt: at, createdById: actor.userId, createdByName: actor.name, updatedAt: at, rev: 1,
    };
    await this.guardStatus(u, ticket, options);
    await u.tickets.insert(ticket);
    await u.history.insert([{ ticketId: id, at, byId: actor.userId, byName: actor.name, field: 'status', from: null, to: status, note: 'angelegt' }]);
    if (steps.length) await u.steps.replace(id, steps);
    return ticket;
  }

  // Wirft 'ticket-duplicate', wenn auf dem Board ein offenes Ticket (nicht Erledigt, nicht „Nicht mehr nötig“) denselben
  // Titel trägt (getrimmt, ohne Rücksicht auf Groß-/Kleinschreibung)
  private async rejectDuplicate(u: BoardUnitOfWork, boardId: number, title: string): Promise<void> {
    const wanted = (title ?? '').trim().toLowerCase();
    if (!wanted) return;
    const same = (await u.tickets.list({ boardIds: [boardId] }))
      .find((t) => t.status !== 'done' && t.status !== 'not_needed' && t.title.trim().toLowerCase() === wanted);
    if (same) throw new HarnessError('ticket-duplicate', `Ein offenes Ticket mit diesem Titel gibt es schon: ${same.id}.`, { existingId: same.id });
  }

  // Liest ein Ticket (Schreibweise egal) und prüft die erwartete Version
  async load(u: BoardUnitOfWork, id: string, rev?: number, forUpdate = false): Promise<Ticket> {
    const canonical = this.ids.canonical(id);
    const ticket = await u.tickets.byId(canonical, forUpdate);
    if (!ticket) throw new HarnessError('ticket-not-found', `Ticket ${canonical} gibt es nicht.`);
    if (rev !== undefined && rev !== ticket.rev) {
      throw new HarnessError('conflict', `Ticket ${canonical} wurde inzwischen geändert (Version ${ticket.rev}, erwartet ${rev}). Bitte neu lesen.`);
    }
    return ticket;
  }

  // Schreibt die neue Fassung mit nächster Version und Zeitstempel und protokolliert die Änderungen
  async save(u: BoardUnitOfWork, before: Ticket, after: Ticket, actor: BoardActor, note: string | null): Promise<Ticket> {
    BoardLimits.actor(actor);
    const at = this.now();
    const next = { ...after, rev: before.rev + 1, updatedAt: at };
    await u.tickets.update(next);
    const entries = this.diff.diff(before, next, actor, at, note);
    if (entries.length) await u.history.insert(entries);
    return next;
  }

  // Anzeigefassung eines Tickets (Spalte, wartet, offene Voraussetzungen, Abhängige)
  async view(u: BoardUnitOfWork, ticket: Ticket): Promise<TicketView> {
    return (await this.views(u, [ticket]))[0];
  }

  // Anzeigefassungen; Voraussetzungen werden über alle Boards aufgelöst, offene Fragen, Schritte und Sessions je Ticket ergänzt
  async views(u: BoardUnitOfWork, tickets: Ticket[]): Promise<TicketView[]> {
    const all = await u.tickets.list({ withoutTexts: true });
    const byId = new Map(all.map((t) => [t.id, t]));
    for (const t of tickets) byId.set(t.id, t);
    const ids = tickets.map((t) => t.id);
    const questions = await this.blocking.openByTicket(u, ids);
    const progress = ids.length ? await u.steps.progress(ids) : {};
    const sessions = ids.length ? await u.ticketSessions.byTickets(ids) : {};
    const known = [...byId.values()];
    return tickets.map((t) => {
      const waiting = this.deps.isWaiting(t, byId);
      return {
        ...t,
        column: this.ordering.column(t, waiting),
        waiting,
        dependsOpen: this.deps.openDependencies(t, byId),
        dependsUnmerged: this.deps.unmergedDependencies(t, byId),
        dependents: all.filter((o) => o.dependsOn.includes(t.id)).map((o) => o.id).sort(),
        unlocks: this.depGraph.unlockedBy(t.id, known, byId),
        waitingOnQuestions: questions.get(t.id) ?? [],
        stepsDone: progress[t.id]?.done ?? 0,
        stepsTotal: progress[t.id]?.total ?? 0,
        sessions: sessions[t.id] ?? [],
      };
    });
  }

  // Sperren beim Statuswechsel: not_needed nur durch den Nutzer, done nur mit Abnahme,
  // in_progress/done nur ohne offene Voraussetzung (force übergeht die erste und die letzte Sperre)
  async checkStatus(u: BoardUnitOfWork, before: Ticket, after: Ticket, options: WriteOptions): Promise<void> {
    if (before.status === after.status) return;
    await this.guardStatus(u, after, options);
  }

  // Die Sperren für den Status eines Tickets (beim Wechsel und beim Anlegen)
  async guardStatus(u: BoardUnitOfWork, after: Ticket, options: WriteOptions): Promise<void> {
    if (after.status === 'not_needed' && !options.byUser && !options.force) {
      throw new HarnessError('not-needed-user', '„Nicht mehr nötig“ entscheidet der Nutzer. Hältst du das Ticket für überflüssig, schreib einen Kommentar; --force nur auf ausdrückliche Anweisung.');
    }
    if (after.status === 'done' && !after.acceptance.trim()) {
      throw new HarnessError('acceptance-missing', `${after.id} hat keine Abnahme. Mit --acceptance angeben, woran man sieht, dass es fertig ist.`);
    }
    if ((after.status === 'in_progress' || after.status === 'done') && !options.force) {
      await this.requireNoOpenDependency(u, after);
    }
  }

  // Wirft, wenn eine Voraussetzung oder eine Frage zum Ticket offen ist
  async requireNoOpenDependency(u: BoardUnitOfWork, ticket: Ticket): Promise<void> {
    const byId = new Map((await u.tickets.list({ withoutTexts: true })).map((t) => [t.id, t]));
    const open = this.deps.openDependencies(ticket, byId);
    if (open.length) {
      throw new HarnessError('dependency-open', `${ticket.id} wartet auf ${open.join(', ')}. Erst die Voraussetzung erledigen.`);
    }
    const questions = (await this.blocking.openByTicket(u, [ticket.id])).get(ticket.id) ?? [];
    if (questions.length) {
      throw new HarnessError('question-open', `${ticket.id} wartet auf Frage ${questions.map((q) => q.id).join(', ')}. Erst die Frage entscheiden lassen.`);
    }
  }

  // Nächste SortOrder am Ende einer Spalte
  async nextOrder(u: BoardUnitOfWork, boardId: number, status: TicketStatus): Promise<number> {
    const max = await u.tickets.maxSortOrder(boardId, status);
    return max === null ? 0 : max + 1;
  }

  // Wendet eine Änderung an und prüft die betroffenen Felder
  private async apply(u: BoardUnitOfWork, before: Ticket, patch: TicketPatch): Promise<Ticket> {
    const after: Ticket = { ...before };
    if (patch.title !== undefined) after.title = this.requireTitle(patch.title);
    if (patch.description !== undefined) after.description = patch.description;
    if (patch.plan !== undefined) after.plan = patch.plan;
    if (patch.handbook !== undefined) after.handbook = patch.handbook;
    if (patch.diagram !== undefined) after.diagram = patch.diagram;
    if (patch.status !== undefined) after.status = patch.status;
    if (patch.priority !== undefined) after.priority = this.requirePriority(patch.priority);
    if (patch.labels !== undefined) after.labels = BoardLabels.normalize(patch.labels);
    if (patch.assignee !== undefined) after.assignee = BoardLimits.text(patch.assignee && patch.assignee.trim() ? patch.assignee.trim() : null, MAX_PERSON_NAME_LENGTH, 'Der Name des Zuständigen');
    if (patch.needsPlanning !== undefined) after.needsPlanning = patch.needsPlanning;
    if (before.status === 'blocked' && after.status !== 'blocked' && patch.blockedReason === undefined) after.blockedReason = null;
    if (patch.blockedReason !== undefined) after.blockedReason = BoardLimits.text(patch.blockedReason && patch.blockedReason.trim() ? patch.blockedReason.trim() : null, MAX_BLOCKED_REASON_LENGTH, 'Der Grund für Blockiert');
    if (patch.acceptance !== undefined) after.acceptance = patch.acceptance;
    if (patch.spec !== undefined) after.spec = this.optional(patch.spec, MAX_SPEC_LENGTH, 'Die Spec');
    if (patch.specTask !== undefined) after.specTask = this.optional(patch.specTask, MAX_SPEC_TASK_LENGTH, 'Die Spec-Aufgabe');
    if (patch.branch !== undefined) after.branch = this.optional(patch.branch, MAX_BRANCH_LENGTH, 'Der Branch');
    if (patch.worktree !== undefined) after.worktree = this.optional(patch.worktree, MAX_WORKTREE_LENGTH, 'Der Worktree-Pfad');
    if (patch.targetBranch !== undefined) {
      after.targetBranch = this.targetRule.normalize(patch.targetBranch);
      // Gesperrt wird nach dem Stand vor der Änderung (before.branch); ein Patch, der Branch und Zielbranch zugleich setzt oder leert, darf also beides ändern, solange vorher kein Branch gesetzt war
      if (after.targetBranch !== before.targetBranch && before.branch) {
        throw new HarnessError('target-branch-locked', `${before.id} läuft schon auf Branch ${before.branch}; der Zielbranch lässt sich nur vor dem Start ändern.`);
      }
    }
    if (patch.touches !== undefined) after.touches = this.cleanList(patch.touches);
    if (patch.consumes !== undefined) after.consumes = this.cleanList(patch.consumes);
    if (patch.produces !== undefined) after.produces = this.cleanList(patch.produces);
    if (patch.dependsOn !== undefined || patch.dependsOnAdd !== undefined || patch.dependsOnRemove !== undefined) {
      const remove = new Set(this.deps.normalize(patch.dependsOnRemove ?? []));
      const base = patch.dependsOn ?? [...before.dependsOn, ...(patch.dependsOnAdd ?? [])];
      after.dependsOn = this.deps.normalize(base).filter((d) => !remove.has(d));
      if (JSON.stringify(after.dependsOn) !== JSON.stringify(before.dependsOn)) this.deps.validate(before.id, after.dependsOn, await this.graph(u));
    }
    if (patch.boardId !== undefined && patch.boardId !== before.boardId) {
      const target = await this.boards.get(u, patch.boardId);
      if (target.projectId !== before.projectId) {
        throw new HarnessError('board-other-project', `Board ${target.id} gehört zu einem anderen Projekt. Tickets wechseln nur zwischen Boards desselben Projekts.`);
      }
      after.boardId = target.id;
    }
    if (after.status !== before.status || after.boardId !== before.boardId) after.sortOrder = await this.nextOrder(u, after.boardId, after.status);
    return after;
  }

  // Alle Tickets mit ihren Voraussetzungen (für die Kreisprüfung)
  private async graph(u: BoardUnitOfWork): Promise<Map<string, string[]>> {
    return new Map((await u.tickets.list({ withoutTexts: true })).map((t) => [t.id, t.dependsOn]));
  }

  // Titel ohne Leerraum am Rand; leer oder zu lang ist nicht erlaubt
  private requireTitle(title: string): string {
    const value = title.trim();
    if (!value) throw new HarnessError('title-empty', 'Der Titel darf nicht leer sein.');
    return BoardLimits.text(value, MAX_TITLE_LENGTH, 'Der Titel');
  }

  // Priorität prüfen
  private requirePriority(priority: string): Priority {
    const found = PRIORITIES.find((p) => p === priority);
    if (!found) throw new HarnessError('priority-invalid', `Priorität „${priority}“ ist ungültig. Erlaubt: ${PRIORITIES.join(', ')}.`);
    return found;
  }

  // Detailfassung: Anzeige plus Kommentare, Verlauf und Schritte
  private async detail(u: BoardUnitOfWork, id: string): Promise<TicketDetail> {
    const ticket = (await u.tickets.byId(id))!;
    return {
      ...(await this.view(u, ticket)),
      comments: await u.comments.list(id), history: await u.history.list(id), steps: await u.steps.list(id),
    };
  }

  // Ersetzt die Schritte nur bei Änderung und vermerkt die neue Zahl im Verlauf
  private async writeSteps(u: BoardUnitOfWork, ticket: Ticket, steps: TicketStep[], actor: BoardActor): Promise<void> {
    const current = await u.steps.list(ticket.id);
    if (JSON.stringify(current) === JSON.stringify(steps)) return;
    await u.steps.replace(ticket.id, steps);
    await u.history.insert([{
      ticketId: ticket.id, at: ticket.updatedAt, byId: actor.userId, byName: actor.name, field: 'steps',
      from: `${current.length} Schritte`, to: `${steps.length} Schritte`, note: null,
    }]);
  }

  // Eine Schritt-Änderung in einer Transaktion: Liste lesen, ändern, ersetzen; bei Änderung neue Version und Verlauf.
  // Mit guard.rev nur, wenn das Ticket seitdem unverändert ist (sonst conflict).
  private changeSteps(
    id: string, actor: BoardActor, guard: StepGuard, edit: (ticketId: string, steps: TicketStep[]) => StepEdit,
  ): Promise<TicketDetail> {
    return this.db.transaction(async (u) => {
      BoardLimits.actor(actor);
      const before = await this.load(u, id, guard.rev, true);
      const current = await u.steps.list(before.id);
      const result = edit(before.id, current);
      if (JSON.stringify(result.steps) !== JSON.stringify(current)) {
        const at = this.now();
        await u.steps.replace(before.id, result.steps);
        await u.tickets.update({ ...before, rev: before.rev + 1, updatedAt: at });
        await u.history.insert([{
          ticketId: before.id, at, byId: actor.userId, byName: actor.name, field: 'step', from: result.from, to: result.to, note: result.note,
        }]);
      }
      return this.detail(u, before.id);
    });
  }

  // Verknüpft eine Session innerhalb der Transaktion; leere oder zu lange Kennungen sind nicht erlaubt
  private async addSession(u: BoardUnitOfWork, ticketId: string, sessionId: string): Promise<boolean> {
    const value = sessionId.trim();
    if (!value) throw new HarnessError('session-invalid', 'Die Kennung der Session fehlt.');
    BoardLimits.text(value, MAX_SESSION_ID_LENGTH, 'Die Kennung der Session');
    return u.ticketSessions.add(ticketId, value, this.now());
  }

  // Schritte ohne Leerraum am Rand; leerer Text oder zu lang ist nicht erlaubt, ein leeres Detail entfällt
  private cleanSteps(steps: TicketStep[]): TicketStep[] {
    return steps.map((s) => {
      const text = s.text.trim();
      if (!text) throw new HarnessError('step-empty', 'Ein Schritt braucht einen Text.');
      BoardLimits.text(text, MAX_STEP_TEXT_LENGTH, 'Der Text eines Schritts');
      return s.detail && s.detail.trim() ? { text, done: Boolean(s.done), detail: s.detail } : { text, done: Boolean(s.done) };
    });
  }

  // Liste ohne Leerraum am Rand und ohne leere Einträge
  private cleanList(values: string[]): string[] {
    return values.map((v) => v.trim()).filter(Boolean);
  }

  // Optionaler Text: leer wird null, Länge wird geprüft
  private optional(value: string | null | undefined, max: number, what: string): string | null {
    return BoardLimits.text(value && value.trim() ? value.trim() : null, max, what);
  }

  // Aktuelle Zeit als ISO-Text
  private now(): string {
    return this.clock().toISOString();
  }
}
