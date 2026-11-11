// Typen der Board-Datenbank (/api/db/…), getrennt von den Datei-Tickets des alten Boards.

export type DbStatus = 'backlog' | 'in_planning' | 'planned' | 'in_progress' | 'blocked' | 'done' | 'not_needed';
export type DbColumnId = DbStatus | 'waiting';
export type DbPriority = 'low' | 'normal' | 'high' | 'urgent';
export type DbNoteState = 'open' | 'taken' | 'dropped';
export type DbNoteFilter = DbNoteState | 'all';

export const DB_STATUSES: DbStatus[] = ['backlog', 'in_planning', 'planned', 'in_progress', 'blocked', 'done', 'not_needed'];
export const DB_PRIORITIES: DbPriority[] = ['urgent', 'high', 'normal', 'low'];
export const DB_COLUMN_LABELS: Record<DbColumnId, string> = {
  backlog: 'Backlog', waiting: 'Wartet', in_planning: 'In Planung', planned: 'Geplant',
  in_progress: 'In Arbeit', blocked: 'Blockiert', done: 'Erledigt', not_needed: 'Nicht mehr nötig',
};
export const DB_PRIORITY_LABELS: Record<DbPriority, string> = { urgent: 'dringend', high: 'hoch', normal: 'normal', low: 'niedrig' };

export interface DbUserDto { id: string; name: string }

export interface DbProjectDto { id: number; key: string; name: string; nextTicketNumber: number; createdAt: string; createdBy: string }

/** Herkunft von „Mein Repo-Pfad“: lokal (board-repos.yaml) oder aus der gemeinsamen DB übernommen (Altbestand, nur eigene Boards) */
export type DbRepoSource = 'local' | 'legacy';

/** `repoPath` ist der eigene Repo-Pfad des Benutzers (lokal); fremde Pfade liefert der Server nicht */
export interface DbBoardDto {
  id: number; projectId: number; ownerId: string; name: string; repoPath: string | null; createdAt: string;
  projectKey: string; projectName: string; ownerName: string; openTickets: number;
  repoSource?: DbRepoSource | null;
}

export interface DbActiveDto { user: DbUserDto; boardId: number | null; board: DbBoardDto | null }

/** Abhakbarer Arbeitsschritt eines Tickets; `detail` ist Markdown */
export interface DbStepDto { text: string; done: boolean; detail?: string }

// Neuer Schritt (at: Nummer ab 1, ohne Angabe ans Ende)
export interface DbStepInput { text: string; detail?: string | null; at?: number }

// Änderung eines Schritts (nicht genannte Felder bleiben; leere Anleitung entfernt sie)
export interface DbStepChange { text?: string; detail?: string | null }

export interface DbTicketDto {
  id: string; projectId: number; boardId: number; number: number; title: string; description: string;
  status: DbStatus; priority: DbPriority; labels: string[]; dependsOn: string[]; sortOrder: number;
  assignee: string | null; needsPlanning: boolean; acceptance: string; plan: string; handbook: string; diagram: string;
  blockedReason: string | null; plannedAt: string | null; plannedBy: string | null;
  planningClaimBy: string | null; planningClaimAt: string | null;
  createdAt: string; createdById: string; createdByName: string; updatedAt: string; rev: number;
  /** Herkunft aus der Spec, Branch und Worktree (nexus board start), Bereiche und Schnittstellen */
  spec: string | null; specTask: string | null; branch: string | null; worktree: string | null;
  /** Zielbranch, in den das Ticket gemergt wird (leer: der im Hauptordner ausgecheckte Branch); nach dem Start nicht mehr änderbar */
  targetBranch: string | null;
  touches: string[]; consumes: string[]; produces: string[];
  /** Fortschritt der Schritte und verknüpfte Claude-Sessions */
  stepsDone: number; stepsTotal: number; sessions: string[];
  /** Anzeige-Spalte („waiting“ bei offener Voraussetzung) */
  column: DbColumnId; waiting: boolean; dependsOpen: string[]; dependents: string[];
  /** Abhängige, deren letzte offene Voraussetzung dieses Ticket ist (werden mit seinem Erledigt bereit) */
  unlocks: string[];
  /** Offene Fragen zu diesem Ticket; sperren In Arbeit, Erledigt und Übernehmen */
  waitingOnQuestions: DbOpenQuestionDto[];
}

/** Karte der Board-Ansicht: statt der langen Texte nur, ob sie vorhanden sind (das Detail lädt sie selbst) */
export interface DbCardDto extends Omit<DbTicketDto, 'description' | 'acceptance' | 'plan' | 'handbook' | 'diagram'> {
  commentCount: number;
  hasDescription: boolean; hasAcceptance: boolean; hasPlan: boolean; hasHandbook: boolean; hasDiagram: boolean;
  /** Wann das Ticket zuletzt auf Erledigt ging; nur bei erledigten Karten gesetzt */
  doneAt?: string | null;
}

export interface DbColumnDto { id: DbColumnId; title: string; tickets: DbCardDto[] }

export interface DbBoardViewDto { boardId: number; columns: DbColumnDto[] }

/** Branches des Board-Repos als Vorschläge für den Zielbranch; `no-repo`: auf diesem Rechner gibt es das Repo nicht */
export interface DbBranchListDto { branches: string[]; reason: 'no-repo' | null }

/** Ergebnis von „Branch anlegen“; `from` nennt die Herkunft eines neu angelegten Branches (sonst null) */
export interface DbCreatedBranchDto { name: string; created: boolean; from: string | null }

/** Umfang eines Abhängigkeitsgraphen */
export type DbGraphScope =
  | { kind: 'ticket'; id: string }
  | { kind: 'spec'; project: string; spec: string }
  | { kind: 'board'; boardId: number };

export interface DbGraphNodeDto {
  id: string; title: string; status: DbStatus; column: DbColumnId; boardId: number; boardName: string; spec: string | null;
  /** Direkter Nachbar außerhalb des Umfangs (andere Spec oder anderes Board) */
  external: boolean;
}

/** Kante „to setzt from voraus“ */
export interface DbGraphEdgeDto { from: string; to: string }

/** GET /api/db/graph */
export interface DbGraphDto {
  scope: DbGraphScope; nodes: DbGraphNodeDto[]; edges: DbGraphEdgeDto[];
  /** Offene Tickets der längsten Kette, von der ersten Voraussetzung bis zum Ziel */
  criticalPath: string[];
  /** Kanten, die einen Kreis schließen (Datenfehler) */
  cycleEdges: DbGraphEdgeDto[];
  hiddenWithoutEdges: number; hiddenDone: number;
}

/** Stand einer Spec aus dem Board; total ohne „nicht mehr nötig“ */
export interface DbSpecProgressDto {
  spec: string; total: number; done: number; blocked: number; waiting: number; inProgress: number; open: number; notNeeded: number;
}

export interface DbCommentDto { id: number; ticketId: string; authorId: string; authorName: string; body: string; createdAt: string; readByAgentAt?: string | null }

export interface DbHistoryDto { ticketId: string; at: string; byId: string; byName: string; field: string; from: string | null; to: string | null; note: string | null }

export interface DbTicketDetailDto extends DbTicketDto { comments: DbCommentDto[]; history: DbHistoryDto[]; steps: DbStepDto[] }

export interface DbNewTicketInput {
  title: string; description?: string; status?: DbStatus; priority?: DbPriority; labels?: string[]; needsPlanning?: boolean; dependsOn?: string[];
  /** Pflicht bei Status „done“ */
  acceptance?: string;
}

/** Änderungswunsch; `rev` ist die gelesene Version (Pflicht) */
export interface DbTicketPatch {
  rev: number; title?: string; description?: string; plan?: string; handbook?: string; diagram?: string;
  status?: DbStatus; priority?: DbPriority; labels?: string[]; assignee?: string | null; needsPlanning?: boolean;
  blockedReason?: string | null; acceptance?: string; dependsOn?: string[]; targetBranch?: string | null;
}

export interface DbMoveInput { status: DbStatus; rev?: number; acceptance?: string }

export interface DbNoteDto {
  id: number; boardId: number; authorId: string; authorName: string;
  /** Leer bei Notizen von vor der Trennung in Titel und Inhalt */
  title: string;
  body: string; source: string | null;
  labels: string[]; seenCount: number; seenIn: string[]; state: DbNoteState; ticketId: string | null; reason: string | null; createdAt: string;
}

/** `force` legt auch an, wenn es eine ähnliche Notiz gibt (sonst Fehler „note-similar“ mit `details.notes`) */
export interface DbNewNoteInput { title: string; body?: string; source?: string; labels?: string[]; force?: boolean }
/** Kandidat aus der Ablehnung „note-similar“ */
export interface DbSimilarNote { id: number; title: string; state: DbNoteState; ticketId: string | null }
export interface DbNoteTakeInput { title?: string; status?: DbStatus; priority?: DbPriority; labels?: string[] }
export interface DbNoteTakeResult { note: DbNoteDto; ticket: DbTicketDto }
/** `repoPath` setzt den eigenen, lokalen Repo-Pfad (nicht die gemeinsame DB) */
export interface DbBoardPatch { name?: string; repoPath?: string | null }
/** Ergebnis von „Von anderem Board holen“: die geholten Tickets in neuer Fassung */
export interface DbImportResult { moved: DbTicketDto[] }

// Fragen auf der Board-Datenbank (/api/db/projects/:key/questions, /api/db/questions/:id)

export type DbQuestionState = 'open' | 'answered' | 'discarded';
export type DbOriginKind = 'human' | 'session' | 'subagent';
export const DB_QUESTION_STATE_LABELS: Record<DbQuestionState, string> = { open: 'offen', answered: 'beantwortet', discarded: 'verworfen' };

export interface DbOpenQuestionDto { id: string; title: string }

export interface DbQuestionOptionDto { id: string; title: string; text: string; pros: string[]; cons: string[]; recommended: boolean }

export interface DbQuestionResearchDto {
  id: number; questionId: string; at: string; byId: string; byName: string; kind: DbOriginKind; context: string; options: DbQuestionOptionDto[];
}

export interface DbQuestionDecisionDto {
  outcome: 'answered' | 'discarded'; option: string | null; text: string | null; reason: string; byId: string; byName: string; at: string;
}

export interface DbQuestionTicketRefDto { id: string; title: string; status: DbStatus; boardId: number }

export interface DbQuestionDto {
  id: string; projectId: number; number: number; projectKey: string; title: string; description: string; state: DbQuestionState;
  occasion: string | null; originKind: DbOriginKind; originName: string; originSession: string | null;
  tags: string[]; ticketIds: string[]; dependsOn: string[]; decision: DbQuestionDecisionDto | null;
  createdAt: string; createdById: string; updatedAt: string; rev: number;
  /** Betroffene Tickets mit Titel und Status */
  tickets: DbQuestionTicketRefDto[];
  /** Offene Abhängigkeiten (Frage offen, Ticket nicht erledigt) */
  waitingOn: Array<{ id: string; state: string }>;
  ready: boolean; researched: boolean; optionCount: number;
  /** Nur in der Liste: betrifft den Benutzer („Eigene“: selbst gestellt oder zu Tickets auf eigenen Boards) – rechnet der Server */
  mine?: boolean;
}

export interface DbQuestionCommentDto { id: number; questionId: string; authorId: string; authorName: string; authorKind: DbOriginKind; body: string; createdAt: string }

export interface DbQuestionHistoryDto { questionId: string; at: string; byId: string; byName: string; byKind: DbOriginKind | null; type: string; text: string }

export interface DbQuestionDetailDto extends DbQuestionDto {
  research: DbQuestionResearchDto[]; comments: DbQuestionCommentDto[]; history: DbQuestionHistoryDto[];
}

/** Zähler je Zustand; `mine` = offene Fragen, die den Benutzer betreffen (Zähler in der Navigation) */
export interface DbQuestionListDto { questions: DbQuestionDto[]; counts: Record<DbQuestionState, number> & { mine: number } }

export interface DbNewQuestionInput {
  title: string; description?: string; tags?: string[]; occasion?: string | null; ticketIds?: string[]; dependsOn?: string[];
}

/** Änderungswunsch; `rev` ist die gelesene Version (Pflicht) */
export interface DbQuestionPatch { rev: number; title?: string; description?: string; tags?: string[]; occasion?: string | null }

/** Entscheidung: genau eines von option und text, dazu die Begründung */
export interface DbDecisionInput { rev: number; option?: string; text?: string; reason: string }

// Zeitleiste eines Tickets (/api/db/tickets/:id/timeline)

export type DbTimelineKind = 'status' | 'claim' | 'run' | 'question' | 'git' | 'comment' | 'content' | 'other';

export interface DbTimelineEventDto {
  at: string;
  kind: DbTimelineKind;
  /** z. B. 'field:status', 'comment', 'dispatch:start', 'run-log', 'commit', 'merge', 'question:decided' */
  type: string;
  actor: { name: string; type: 'person' | 'agent' };
  field?: string; from?: string | null; to?: string | null; note?: string | null;
  text?: string;
  ref?: { questionId?: string; commit?: string; runLog?: string; profile?: string; exitCode?: number | null };
}

export interface DbTicketTimelineDto {
  ticketId: string;
  events: DbTimelineEventDto[];
  /** Rechnerlokale Quellen: Lauf-Protokolle des Dispatchers und Commits im Repo des Boards */
  sources: { runs: 'ok' | 'unavailable'; git: 'ok' | 'unavailable' | 'error'; gitMessage?: string };
}

// Suche über alle Boards (/api/db/search)

export type DbSearchKind = 'ticket' | 'question' | 'note';
export type DbSearchField = 'title' | 'description' | 'comment' | 'plan' | 'handbook' | 'acceptance' | 'decision' | 'question-comment' | 'note';

export interface DbSearchHitDto {
  kind: DbSearchKind;
  id: string;
  /** Ticket zum Öffnen (Ticket, Kommentar, übernommene Notiz) */
  ticketId: string | null;
  field: DbSearchField;
  title: string;
  project: { id: number; key: string; name: string };
  board: { id: number; name: string } | null;
  status: string;
  snippet: string;
  /** Trefferbereiche [Anfang, Ende) im Ausschnitt */
  matches: Array<[number, number]>;
  at: string | null;
}

export interface DbSearchResultDto { query: string; terms: string[]; hits: DbSearchHitDto[]; truncated: boolean }
