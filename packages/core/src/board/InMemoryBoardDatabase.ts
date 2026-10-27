import { DISPATCH_MARKERS } from '../dispatch/DispatchConfig.js';
import { HarnessError } from '../errors/HarnessError.js';
import type { BoardDatabase, BoardUnitOfWork } from './BoardDatabase.js';
import type { BoardQuestion, BoardQuestionComment, BoardQuestionHistoryEntry, BoardQuestionResearch, BoardQuestionResearchSummary } from './BoardQuestionTypes.js';
import { InMemoryBoardSearch } from './InMemoryBoardSearch.js';
import type { Board, BoardUser, Comment, Deletion, HistoryEntry, Note, Project, Ticket, TicketStep } from './BoardTypes.js';
import type { DispatchCommentCounts, DispatchConfigVersion, DispatchRun } from './DispatchRunTypes.js';

interface State {
  users: BoardUser[];
  projects: Project[];
  boards: Board[];
  tickets: Ticket[];
  comments: Comment[];
  steps: Array<{ ticketId: string; steps: TicketStep[] }>;
  ticketSessions: Array<{ ticketId: string; sessionId: string; addedAt: string }>;
  history: HistoryEntry[];
  notes: Note[];
  deletions: Deletion[];
  questions: BoardQuestion[];
  questionResearch: BoardQuestionResearch[];
  questionComments: BoardQuestionComment[];
  questionHistory: BoardQuestionHistoryEntry[];
  dispatchRuns: DispatchRun[];
  dispatchConfigVersions: DispatchConfigVersion[];
  nextId: number;
}

// Was eine Transaktion berührt: Boards (changeMarker) und Projekte mit geänderten Fragen (questionMarker)
interface Touched {
  boards: Set<number>;
  projects: Set<number>;
}

// Board-Datenbank im Speicher für Tests: gleiche Semantik wie SQL Server (Transaktion, Rev-Prüfung).
export class InMemoryBoardDatabase implements BoardDatabase {
  private state: State = {
    users: [], projects: [], boards: [], tickets: [], comments: [], steps: [], ticketSessions: [], history: [], notes: [], deletions: [],
    questions: [], questionResearch: [], questionComments: [], questionHistory: [], dispatchRuns: [], dispatchConfigVersions: [], nextId: 1,
  };
  private queue: Promise<unknown> = Promise.resolve();
  private readonly search = new InMemoryBoardSearch();
  // Änderungszähler je Board (Grundlage für changeMarker)
  private readonly markers = new Map<number, number>();
  // Änderungszähler der Fragen je Projekt (Grundlage für questionMarker)
  private readonly questionMarkers = new Map<number, number>();

  // Führt die Arbeit auf einer Kopie aus und übernimmt sie nur bei Erfolg; berührte Boards und Projekte zählen weiter
  transaction<T>(work: (uow: BoardUnitOfWork) => Promise<T>): Promise<T> {
    const run = this.queue.then(async () => {
      const draft = structuredClone(this.state);
      const touched: Touched = { boards: new Set(), projects: new Set() };
      const result = await work(this.unitOfWork(draft, touched));
      this.state = draft;
      for (const id of touched.boards) this.markers.set(id, (this.markers.get(id) ?? 0) + 1);
      for (const id of touched.projects) this.questionMarkers.set(id, (this.questionMarkers.get(id) ?? 0) + 1);
      return structuredClone(result);
    });
    this.queue = run.catch(() => undefined);
    return run;
  }

  // Zähler des Boards als Text; steigt mit jeder erfolgreichen Transaktion, die das Board berührt
  async changeMarker(boardId: number): Promise<string> {
    return String(this.markers.get(boardId) ?? 0);
  }

  // Zähler der Fragen eines Projekts als Text
  async questionMarker(projectId: number): Promise<string> {
    return String(this.questionMarkers.get(projectId) ?? 0);
  }

  // Nichts zu schließen
  async close(): Promise<void> {}

  // Baut die Repositories über einem Zustand; schreibende Aufrufe merken sich betroffene Boards und Projekte
  private unitOfWork(s: State, touched: Touched): BoardUnitOfWork {
    const nextId = () => s.nextId++;
    const copy = <T>(value: T): T => structuredClone(value);
    const touch = (boardId: number | undefined) => {
      if (boardId !== undefined) touched.boards.add(boardId);
    };
    const boardOf = (ticketId: string) => s.tickets.find((t) => t.id === ticketId)?.boardId;
    // Eine geänderte Frage berührt ihr Projekt und die Boards der betroffenen Tickets (vorher und nachher)
    const touchQuestion = (...versions: Array<BoardQuestion | undefined>) => {
      for (const q of versions) {
        if (!q) continue;
        touched.projects.add(q.projectId);
        q.ticketIds.forEach((id) => touch(boardOf(id)));
      }
    };
    return {
      users: {
        upsert: async (user) => {
          const found = s.users.find((u) => u.id === user.id);
          if (found) found.name = user.name;
          else s.users.push({ ...user });
        },
        byId: async (id) => copy(s.users.find((u) => u.id === id) ?? null),
        list: async () => copy(s.users),
      },
      projects: {
        list: async () => copy([...s.projects].sort((a, b) => a.key.localeCompare(b.key))),
        byId: async (id) => copy(s.projects.find((p) => p.id === id) ?? null),
        byKey: async (key) => copy(s.projects.find((p) => p.key === key) ?? null),
        insert: async (input) => {
          if (s.projects.some((p) => p.key === input.key)) throw new HarnessError('project-exists', `Projekt ${input.key} gibt es schon.`);
          const project = { ...input, id: nextId(), nextTicketNumber: 1, nextQuestionNumber: 1 };
          s.projects.push(project);
          return copy(project);
        },
        rename: async (id, name) => {
          const p = s.projects.find((x) => x.id === id);
          if (p) p.name = name;
        },
        takeTicketNumber: async (id) => {
          const p = s.projects.find((x) => x.id === id)!;
          return p.nextTicketNumber++;
        },
        takeQuestionNumber: async (id) => {
          const p = s.projects.find((x) => x.id === id)!;
          return p.nextQuestionNumber++;
        },
      },
      boards: {
        list: async (filter = {}) => copy(s.boards.filter((b) =>
          (filter.projectId === undefined || b.projectId === filter.projectId) && (filter.ownerId === undefined || b.ownerId === filter.ownerId))),
        byId: async (id) => copy(s.boards.find((b) => b.id === id) ?? null),
        insert: async (input) => {
          const board = { ...input, id: nextId() };
          s.boards.push(board);
          touch(board.id);
          return copy(board);
        },
        update: async (board) => {
          const i = s.boards.findIndex((b) => b.id === board.id);
          if (i >= 0) s.boards[i] = { ...board };
          touch(board.id);
        },
        delete: async (id) => {
          s.boards = s.boards.filter((b) => b.id !== id);
          s.notes = s.notes.filter((n) => n.boardId !== id);
          touch(id);
        },
      },
      tickets: {
        byId: async (id) => copy(s.tickets.find((t) => t.id === id) ?? null),
        list: async (filter = {}) => copy(s.tickets.filter((t) =>
          (!filter.boardIds || filter.boardIds.includes(t.boardId)) && (!filter.ids || filter.ids.includes(t.id)))),
        insert: async (ticket) => {
          s.tickets.push(copy(ticket));
          touch(ticket.boardId);
        },
        update: async (ticket) => {
          const i = s.tickets.findIndex((t) => t.id === ticket.id);
          if (i < 0 || s.tickets[i].rev !== ticket.rev - 1) {
            throw new HarnessError('conflict', `Ticket ${ticket.id} wurde inzwischen geändert. Bitte neu lesen.`);
          }
          touch(s.tickets[i].boardId);
          touch(ticket.boardId);
          s.tickets[i] = copy(ticket);
        },
        // Wie FK_QuestionTickets_Tickets in SQL (ohne Kaskade): Ticket mit verknüpften Fragen ist nicht löschbar
        delete: async (id) => {
          if (s.questions.some((q) => q.ticketIds.includes(id))) {
            throw new HarnessError('conflict', `Ticket ${id} ist noch mit Fragen verknüpft.`);
          }
          touch(boardOf(id));
          s.tickets = s.tickets.filter((t) => t.id !== id);
          s.comments = s.comments.filter((c) => c.ticketId !== id);
          s.history = s.history.filter((h) => h.ticketId !== id);
          s.steps = s.steps.filter((x) => x.ticketId !== id);
          s.ticketSessions = s.ticketSessions.filter((x) => x.ticketId !== id);
        },
        dependents: async (id) => s.tickets.filter((t) => t.dependsOn.includes(id)).map((t) => t.id).sort(),
        removeDependencyOn: async (id) => {
          const changed: string[] = [];
          for (const t of s.tickets) {
            if (t.dependsOn.includes(id)) {
              t.dependsOn = t.dependsOn.filter((d) => d !== id);
              changed.push(t.id);
              touch(t.boardId);
            }
          }
          return changed.sort();
        },
        maxSortOrder: async (boardId, status) => {
          const orders = s.tickets.filter((t) => t.boardId === boardId && t.status === status).map((t) => t.sortOrder);
          return orders.length ? Math.max(...orders) : null;
        },
        openCountByBoard: async (boardIds) => {
          const counts: Record<number, number> = {};
          for (const t of s.tickets) {
            if (!boardIds.includes(t.boardId) || t.status === 'done' || t.status === 'not_needed') continue;
            counts[t.boardId] = (counts[t.boardId] ?? 0) + 1;
          }
          return counts;
        },
      },
      comments: {
        list: async (ticketId) => copy(s.comments.filter((c) => c.ticketId === ticketId)),
        insert: async (input) => {
          const comment: Comment = { ...input, readByAgentAt: input.readByAgentAt ?? null, id: nextId() };
          s.comments.push(comment);
          touch(boardOf(input.ticketId));
          return copy(comment);
        },
        countByBoard: async (boardId) => {
          const ids = new Set(s.tickets.filter((t) => t.boardId === boardId).map((t) => t.id));
          const counts: Record<string, number> = {};
          for (const c of s.comments) if (ids.has(c.ticketId)) counts[c.ticketId] = (counts[c.ticketId] ?? 0) + 1;
          return counts;
        },
        unreadByAgent: async (ticketIds) =>
          copy(s.comments.filter((c) => ticketIds.includes(c.ticketId) && c.readByAgentAt === null).sort((a, b) => a.id - b.id)),
        markReadByAgent: async (ids, at) => {
          for (const c of s.comments) {
            if (ids.includes(c.id) && c.readByAgentAt === null) {
              c.readByAgentAt = at;
              touch(boardOf(c.ticketId));
            }
          }
        },
      },
      steps: {
        list: async (ticketId) => copy(s.steps.find((x) => x.ticketId === ticketId)?.steps ?? []),
        replace: async (ticketId, steps) => {
          s.steps = s.steps.filter((x) => x.ticketId !== ticketId);
          if (steps.length) s.steps.push({ ticketId, steps: copy(steps) });
          touch(boardOf(ticketId));
        },
        setDone: async (ticketId, position, done) => {
          const step = s.steps.find((x) => x.ticketId === ticketId)?.steps[position];
          if (!step) return false;
          step.done = done;
          touch(boardOf(ticketId));
          return true;
        },
        progress: async (ticketIds) => {
          const result: Record<string, { done: number; total: number }> = {};
          for (const x of s.steps) {
            if (ticketIds.includes(x.ticketId)) result[x.ticketId] = { done: x.steps.filter((st) => st.done).length, total: x.steps.length };
          }
          return result;
        },
      },
      ticketSessions: {
        add: async (ticketId, sessionId, at) => {
          if (s.ticketSessions.some((x) => x.ticketId === ticketId && x.sessionId === sessionId)) return false;
          s.ticketSessions.push({ ticketId, sessionId, addedAt: at });
          touch(boardOf(ticketId));
          return true;
        },
        byTickets: async (ticketIds) => {
          const result: Record<string, string[]> = {};
          const sorted = [...s.ticketSessions].sort((a, b) => a.addedAt.localeCompare(b.addedAt) || a.sessionId.localeCompare(b.sessionId));
          for (const x of sorted) if (ticketIds.includes(x.ticketId)) (result[x.ticketId] ??= []).push(x.sessionId);
          return result;
        },
        ticketsOf: async (sessionId) => [...new Set(s.ticketSessions.filter((x) => x.sessionId === sessionId).map((x) => x.ticketId))].sort(),
      },
      history: {
        list: async (ticketId) => copy(s.history.filter((h) => h.ticketId === ticketId)),
        insert: async (entries) => {
          s.history.push(...copy(entries));
        },
      },
      notes: {
        list: async (filter) => copy(s.notes.filter((n) =>
          n.boardId === filter.boardId && (!filter.states || filter.states.includes(n.state)) && (filter.source === undefined || n.source === filter.source))),
        // forUpdate entfällt: Transaktionen laufen hier ohnehin nacheinander
        byId: async (id) => copy(s.notes.find((n) => n.id === id) ?? null),
        insert: async (input) => {
          const note = { ...input, id: nextId() };
          s.notes.push(note);
          touch(note.boardId);
          return copy(note);
        },
        update: async (note) => {
          const i = s.notes.findIndex((n) => n.id === note.id);
          if (i >= 0) s.notes[i] = copy(note);
          touch(note.boardId);
        },
      },
      deletions: {
        insert: async (d) => {
          s.deletions.push(copy(d));
        },
        list: async () => copy(s.deletions),
      },
      questions: {
        // forUpdate entfällt: Transaktionen laufen hier ohnehin nacheinander
        byId: async (id) => copy(s.questions.find((q) => q.id === id) ?? null),
        list: async (filter = {}) => copy(s.questions
          .filter((q) =>
            (filter.projectId === undefined || q.projectId === filter.projectId) &&
            (!filter.ids || filter.ids.includes(q.id)) &&
            (!filter.states || filter.states.includes(q.state)) &&
            (!filter.ticketIds || q.ticketIds.some((t) => filter.ticketIds!.includes(t))))
          .sort((a, b) => a.projectId - b.projectId || a.number - b.number)),
        insert: async (question) => {
          if (s.questions.some((q) => q.id === question.id)) throw new HarnessError('conflict', `Frage ${question.id} gibt es schon.`);
          s.questions.push({ ...copy(question), ticketIds: [...question.ticketIds].sort() });
          touchQuestion(question);
        },
        update: async (question) => {
          const i = s.questions.findIndex((q) => q.id === question.id);
          if (i < 0 || s.questions[i].rev !== question.rev - 1) {
            throw new HarnessError('conflict', `Frage ${question.id} wurde inzwischen geändert. Bitte neu lesen.`);
          }
          touchQuestion(s.questions[i], question);
          s.questions[i] = copy(question);
        },
        idsReferencingTicket: async (ticketId) =>
          s.questions.filter((q) => q.ticketIds.includes(ticketId) || q.dependsOn.includes(ticketId)).map((q) => q.id).sort(),
        removeTicket: async (ticketId) => {
          const changed: string[] = [];
          for (const q of s.questions) {
            if (!q.ticketIds.includes(ticketId) && !q.dependsOn.includes(ticketId)) continue;
            touchQuestion(q);
            q.ticketIds = q.ticketIds.filter((t) => t !== ticketId);
            q.dependsOn = q.dependsOn.filter((d) => d !== ticketId);
            changed.push(q.id);
          }
          return changed.sort();
        },
      },
      questionResearch: {
        list: async (questionId) => copy(s.questionResearch.filter((r) => r.questionId === questionId)),
        insert: async (input) => {
          const research = { ...copy(input), id: nextId() };
          s.questionResearch.push(research);
          return copy(research);
        },
        summaries: async (questionIds) => {
          const result: Record<string, BoardQuestionResearchSummary> = {};
          // In Reihenfolge des Anlegens: die letzte Recherche je Frage bestimmt die Zahl der Optionen
          for (const r of s.questionResearch) {
            if (!questionIds.includes(r.questionId)) continue;
            result[r.questionId] = { count: (result[r.questionId]?.count ?? 0) + 1, optionCount: r.options.length };
          }
          return result;
        },
      },
      questionComments: {
        list: async (questionId) => copy(s.questionComments.filter((c) => c.questionId === questionId)),
        insert: async (input) => {
          const comment = { ...copy(input), id: nextId() };
          s.questionComments.push(comment);
          return copy(comment);
        },
      },
      questionHistory: {
        list: async (questionId) => copy(s.questionHistory.filter((h) => h.questionId === questionId)),
        insert: async (entries) => {
          s.questionHistory.push(...copy(entries));
        },
      },
      search: {
        find: async (request) => copy(this.search.find(s, request)),
      },
      dispatchRuns: {
        insert: async (run) => {
          if (s.dispatchRuns.some((r) => r.id === run.id)) return false;
          s.dispatchRuns.push(copy(run));
          return true;
        },
        list: async (filter) => copy(s.dispatchRuns
          .filter((r) => r.projectId === filter.projectId && (filter.boardId === undefined || r.boardId === filter.boardId) &&
            r.startedAt >= filter.from && r.startedAt < filter.to)
          .sort((a, b) => a.startedAt.localeCompare(b.startedAt) || a.id.localeCompare(b.id))),
        listAll: async (from, to) => copy(s.dispatchRuns
          .filter((r) => r.startedAt >= from && r.startedAt < to)
          .sort((a, b) => a.startedAt.localeCompare(b.startedAt) || a.id.localeCompare(b.id))),
        firstStarted: async (projectId, source) =>
          s.dispatchRuns.filter((r) => r.projectId === projectId && r.source === source).map((r) => r.startedAt).sort()[0] ?? null,
        existing: async (ids) => s.dispatchRuns.filter((r) => ids.includes(r.id)).map((r) => r.id),
        recordConfigVersion: async (version) => {
          if (s.dispatchConfigVersions.some((v) => v.hash === version.hash)) return false;
          s.dispatchConfigVersions.push(copy(version));
          return true;
        },
        listConfigVersions: async (from, to) => copy(s.dispatchConfigVersions
          .filter((v) => v.firstSeenAt >= from && v.firstSeenAt < to)
          .sort((a, b) => a.firstSeenAt.localeCompare(b.firstSeenAt) || a.hash.localeCompare(b.hash))),
      },
      dispatchStats: {
        doneTicketIds: async (filter) => {
          const tickets = new Set(s.tickets
            .filter((t) => t.projectId === filter.projectId && (filter.boardId === undefined || t.boardId === filter.boardId)).map((t) => t.id));
          const lastDone = new Map<string, string>();
          for (const h of s.history) {
            if (h.field !== 'status' || h.to !== 'done' || !tickets.has(h.ticketId)) continue;
            const seen = lastDone.get(h.ticketId);
            if (!seen || h.at > seen) lastDone.set(h.ticketId, h.at);
          }
          return [...lastDone].filter(([, at]) => at >= filter.from && at < filter.to).map(([id]) => id).sort();
        },
        statusHistory: async (ticketIds) => copy(s.history.filter((h) => (h.field === 'status' || h.field === 'blockedReason') && ticketIds.includes(h.ticketId))),
        dispatchCommentCounts: async (ticketIds) => {
          const result: Record<string, DispatchCommentCounts> = {};
          for (const c of s.comments) {
            if (!ticketIds.includes(c.ticketId)) continue;
            const start = c.body.startsWith(DISPATCH_MARKERS.START);
            const abort = c.body.startsWith(DISPATCH_MARKERS.ABORT);
            if (!start && !abort) continue;
            const entry = (result[c.ticketId] ??= { starts: 0, aborts: 0 });
            if (start) entry.starts += 1;
            else entry.aborts += 1;
          }
          return result;
        },
        dispatchComments: async (projectId) => {
          const ids = new Set(s.tickets.filter((t) => t.projectId === projectId).map((t) => t.id));
          return copy(s.comments.filter((c) => ids.has(c.ticketId) && c.body.startsWith('[dispatch] ')).sort((a, b) => a.id - b.id));
        },
      },
    };
  }
}
