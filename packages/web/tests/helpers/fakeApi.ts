import type { ApiClient } from '../../src/api/ApiClient.js';
import type { DispatchStatusDto, SuggestionDto, SuggestionListDto } from '../../src/api/dispatchTypes.js';
import { FakeEventSource } from './FakeEventSource.js';
import type { ProjectDto } from '../../src/api/types.js';
import {
  DB_COLUMN_LABELS, type DbActiveDto, type DbBoardDto, type DbBoardViewDto, type DbCardDto, type DbColumnId, type DbProjectDto,
  type DbQuestionDetailDto, type DbQuestionDto, type DbQuestionListDto, type DbStatus, type DbTicketDetailDto,
  type DbHistoryDto, type DbTicketTimelineDto,
} from '../../src/api/dbTypes.js';
import { TEST_WORKFLOW_PHASES } from './workflowPhases.js';
import { INBOX_KINDS, type InboxItemDto, type InboxListDto } from '../../src/api/inboxTypes.js';

export const projects: ProjectDto[] = [
  { id: 'p1', name: 'Demo', path: '/demo', reachable: true },
  { id: 'p2', name: 'Zweites', path: '/zweites', reachable: true },
];

export const DB_COLUMNS: DbColumnId[] = ['backlog', 'waiting', 'in_planning', 'planned', 'in_progress', 'blocked', 'done', 'not_needed'];

export const dbProjects: DbProjectDto[] = [
  { id: 1, key: 'ARC', name: 'Arches', nextTicketNumber: 10, createdAt: '2026-10-01T08:00:00.000Z', createdBy: 'u1' },
  { id: 2, key: 'OTH', name: 'Other', nextTicketNumber: 1, createdAt: '2026-10-01T08:00:00.000Z', createdBy: 'u1' },
];

export const dbBoards: DbBoardDto[] = [
  { id: 1, projectId: 1, ownerId: 'u1', name: 'Sprint', repoPath: null, createdAt: '2026-10-01T08:00:00.000Z', projectKey: 'ARC', projectName: 'Arches', ownerName: 'Florian', openTickets: 2 },
  { id: 2, projectId: 1, ownerId: 'u1', name: 'Nächster', repoPath: null, createdAt: '2026-10-01T08:00:00.000Z', projectKey: 'ARC', projectName: 'Arches', ownerName: 'Florian', openTickets: 0 },
  { id: 3, projectId: 2, ownerId: 'u1', name: 'Fremd', repoPath: null, createdAt: '2026-10-01T08:00:00.000Z', projectKey: 'OTH', projectName: 'Other', ownerName: 'Florian', openTickets: 0 },
];

// Benutzer und aktives Board (Standard: Board 1)
export function dbActive(boardId: number | null = 1): DbActiveDto {
  return { user: { id: 'u1', name: 'Florian' }, boardId, board: dbBoards.find((b) => b.id === boardId) ?? null };
}

// Karte eines DB-Tickets; nicht genannte Felder haben Standardwerte (Spalte = Status)
export function dbCard(id: string, title: string, status: DbStatus = 'backlog', extra: Partial<DbCardDto> = {}): DbCardDto {
  const number = Number(id.split('-')[1] ?? 0);
  return {
    id, projectId: 1, boardId: 1, number, title, status, priority: 'normal', labels: [], dependsOn: [], sortOrder: number,
    assignee: null, needsPlanning: false, blockedReason: null,
    plannedAt: null, plannedBy: null, planningClaimBy: null, planningClaimAt: null,
    spec: null, specTask: null, branch: null, worktree: null, targetBranch: null, touches: [], consumes: [], produces: [], stepsDone: 0, stepsTotal: 0, sessions: [],
    createdAt: '2026-10-13T10:00:00.000Z', createdById: 'u1', createdByName: 'Florian', updatedAt: '2026-10-13T10:00:00.000Z', rev: 1,
    column: status, waiting: false, dependsOpen: [], dependents: [], unlocks: [], waitingOnQuestions: [], commentCount: 0,
    hasDescription: false, hasAcceptance: false, hasPlan: false, hasHandbook: false, hasDiagram: false, ...extra,
  };
}

// Board-Ansicht mit allen acht Spalten; Karten landen in ihrer Spalte
export function dbView(cards: DbCardDto[], boardId = 1): DbBoardViewDto {
  return { boardId, columns: DB_COLUMNS.map((id) => ({ id, title: DB_COLUMN_LABELS[id], tickets: cards.filter((c) => c.column === id) })) };
}

// Ticketdetail aus einer Karte; die langen Texte sind leer, solange extra sie nicht setzt
export function dbDetail(card: DbCardDto, extra: Partial<DbTicketDetailDto> = {}): DbTicketDetailDto {
  const { commentCount: _count, hasDescription: _d, hasAcceptance: _a, hasPlan: _p, hasHandbook: _h, hasDiagram: _g, ...ticket } = card;
  return { ...ticket, description: '', acceptance: '', plan: '', handbook: '', diagram: '', comments: [], history: [], steps: [], ...extra };
}

// Zustand des Dispatchers (Standard: ausgeschaltet, nichts läuft)
export function dispatchStatus(extra: Partial<DispatchStatusDto> = {}): DispatchStatusDto {
  return {
    enabled: false, state: 'off', message: 'Dispatcher ist ausgeschaltet', pausedUntil: null, wip: 1, active: 0, runs: [], merges: [],
    boardId: 1, boardInScope: true, tickets: [], ticketsError: null, lastTick: null, lastErrors: [], configFile: '/home/.harness/dispatch.yaml',
    configError: null, agentPrefix: 'junior-', ...extra,
  };
}

// Vorschlag „Schritte zu knapp“ mit überschreibbaren Feldern
export function suggestion(over: Partial<SuggestionDto> = {}): SuggestionDto {
  return {
    id: 'aaaaaaaaaaaaaaaa', fingerprint: 'rule:turns-too-low:standard:x', source: 'rule', kind: 'turns-too-low',
    title: 'Schritte zu knapp · standard', reason: '2 von 10 Läufen (20 %) mit „standard“ brachen ab.', target: { profile: 'standard' },
    patch: [{ path: 'profiles.standard.maxTurns', from: 80, to: 120 }], weight: 0.4, evidence: { runs: 10, hits: 2, rate: 0.2 }, ...over,
  };
}

// Vorschlagsliste ohne Einträge; einzelne Felder überschreibbar
export function suggestionList(over: Partial<SuggestionListDto> = {}): SuggestionListDto {
  return {
    configHash: 'hash1', basis: { runs: 0, projects: 0, since: '2026-04-25T00:00:00.000Z' }, suggestions: [], dismissed: [], insufficient: [],
    analysis: null, ...over,
  };
}

// Ein ApiClient aus Teilimplementierungen; alles andere wirft, damit unerwartete Aufrufe auffallen
export function fakeApi(overrides: Partial<Record<keyof ApiClient, unknown>> = {}): ApiClient {
  const base: Record<string, unknown> = {
    getProjects: async () => projects,
    getClaudeUsage: async () => ({ available: false, reason: 'no-login', plan: null, fiveHour: null, sevenDay: null, fetchedAt: null }),
    getDbActive: async () => dbActive(),
    getDbProjects: async () => dbProjects,
    getDbBoards: async () => dbBoards,
    getDbBoardView: async () => dbView([]),
    getDbBranches: async () => ({ branches: ['main', 'release/1'], reason: null }),
    getDbNotes: async () => [],
    getInbox: async () => inboxList([]),
    getDispatchStatus: async () => dispatchStatus(),
    getDispatchSuggestions: async () => suggestionList(),
    getQuestionResearch: async (id: string) => ({ questionId: id, run: null, pausedUntil: null, lastStartedAt: null, count: 0 }),
    getTicketPlanning: async (id: string) => ({ ticketId: id, run: null, pausedUntil: null }),
    getDispatchRuns: async () => [],
    getDbTicketTimeline: async (id: string) => ({ ticketId: id, events: [], sources: { runs: 'ok', git: 'ok' } }),
    searchDb: async (q: string) => ({ query: q, terms: [q], hits: [], truncated: false }),
    getDispatchConfig: async () => ({ file: '/home/.harness/dispatch.yaml', exists: false, raw: {}, config: null, error: null }),
    dispatchPromptPreview: async (kind: string, template?: string | null, id?: string) => ({ kind, id: id ?? null, sample: !id, prompt: template ?? 'Standardvorlage' }),
    getBoardConfig: async () => ({
      configured: false, file: '/home/.harness/board.yaml', provider: 'mssql' as const, dbFile: '/home/.harness/board.db',
      server: '', port: 1433, database: 'AiHarness', user: '',
      hasPassword: false, encrypt: true, trustServerCertificate: true, userId: 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa', userName: '', activeBoard: null,
    }),
    getDocs: async () => ({ source: { root: '/demo', boardId: 1, boardName: 'Sprint', reason: null }, docs: [] }),
    getDocTickets: async () => [],
    openDoc: async () => ({ ok: true }),
    cancelTicketDrafts: async () => ({ cancelled: true }),
    getSavedTicketDrafts: async () => ({ result: null }),
    saveTicketDrafts: async (_pid: string, _path: string, drafts: unknown[]) => ({ result: { drafts, dropped: [], costUsd: null, at: '' } }),
    discardTicketDrafts: async () => ({ ok: true }),
    getSpecProgress: async () => [],
    getDbSpecProgress: async () => [],
    getSessions: async () => [],
    getFavorites: async () => [],
    getUserSettings: async () => ({ notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false }),
    getDbQuestions: async () => dbQuestionList([]),
    getWorkflow: async () => ({ source: 'standard', phases: TEST_WORKFLOW_PHASES }),
    getCustomizations: async () => ({ state: 'unknown' }),
    openEvents: () => new FakeEventSource(),
  };
  return new Proxy({ ...base, ...overrides } as object, {
    get(target, prop: string) {
      if (prop in target) return (target as Record<string, unknown>)[prop];
      return () => {
        throw new Error(`Unerwarteter Aufruf: ${prop}`);
      };
    },
  }) as ApiClient;
}

// Frage der Board-Datenbank; nicht genannte Felder haben Standardwerte (offen, bereit, von einer Session, eigene Frage)
export function dbQuestion(id: string, title: string, extra: Partial<DbQuestionDto> = {}): DbQuestionDto {
  return {
    id, projectId: 1, number: Number(id.split('-F')[1] ?? 0), projectKey: id.split('-')[0], title, description: '', state: 'open', occasion: null,
    originKind: 'session', originName: 'claude', originSession: null, tags: [], ticketIds: [], dependsOn: [], decision: null,
    createdAt: '2026-10-14T10:00:00.000Z', createdById: 'u1', updatedAt: '2026-10-14T10:00:00.000Z', rev: 1,
    tickets: [], waitingOn: [], ready: true, researched: false, optionCount: 0, mine: true, ...extra,
  };
}

// Detail einer Frage ohne Recherchen, Kommentare und Verlauf
export function dbQuestionDetail(question: DbQuestionDto, extra: Partial<DbQuestionDetailDto> = {}): DbQuestionDetailDto {
  return { ...question, research: [], comments: [], history: [], ...extra };
}

// Liste mit Zählern je Zustand und der offenen eigenen (wie der Server)
export function dbQuestionList(questions: DbQuestionDto[]): DbQuestionListDto {
  const counts = { open: 0, answered: 0, discarded: 0, mine: 0 };
  questions.forEach((q) => {
    counts[q.state] += 1;
    if (q.state === 'open' && q.mine) counts.mine += 1;
  });
  return { questions, counts };
}

// Zeitleiste aus Verlaufseinträgen (wie der Server sie aus board.History bildet; Art hier vereinfacht)
export function timelineFrom(history: DbHistoryDto[], ticketId = 'ARC-0001'): DbTicketTimelineDto {
  return {
    ticketId,
    events: history.map((h) => ({
      at: h.at, kind: h.field === 'question' ? 'question' : h.field === 'status' ? 'status' : 'other', type: `field:${h.field}`,
      actor: { name: h.byName, type: 'person' }, field: h.field, from: h.from, to: h.to, note: h.note,
    })),
    sources: { runs: 'ok', git: 'ok' },
  };
}

// Eintrag von „Meine Aufgaben“; nicht genannte Felder haben Standardwerte (blockiertes Ticket auf Board 1)
export function inboxItem(key: string, kind: InboxItemDto['kind'], title: string, extra: Partial<InboxItemDto> = {}): InboxItemDto {
  const [type, id] = key.split(':');
  return {
    key, kind, title, detail: null, boardId: 1, boardName: 'Sprint', projectKey: 'ARC',
    ticketId: type === 'ticket' ? id : null, questionId: type === 'question' ? id : null, noteId: type === 'note' ? Number(id) : null,
    waitingTickets: 0, priority: null, since: '2026-10-19T10:00:00.000Z', rank: INBOX_KINDS.indexOf(kind) + 1, ...extra,
  };
}

// Liste mit Zählern je Art (wie der Server)
export function inboxList(items: InboxItemDto[]): InboxListDto {
  const counts = Object.fromEntries(INBOX_KINDS.map((k) => [k, items.filter((i) => i.kind === k).length])) as InboxListDto['counts'];
  return { items, counts, total: items.length };
}