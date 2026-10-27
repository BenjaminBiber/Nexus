import type { QuestionState } from '../questions/Question.js';
import type { SearchRequest, SearchRows } from './BoardSearchTypes.js';
import type {
  BoardQuestion, BoardQuestionComment, BoardQuestionHistoryEntry, BoardQuestionResearch, BoardQuestionResearchSummary,
} from './BoardQuestionTypes.js';
import type {
  Board, BoardUser, Comment, Deletion, HistoryEntry, Note, NoteState, Project, StepProgress, Ticket, TicketStatus, TicketStep,
} from './BoardTypes.js';
import type { DispatchCommentCounts, DispatchConfigVersion, DispatchRun, DispatchRunFilter, DispatchRunSource } from './DispatchRunTypes.js';

export interface UserRepository {
  // Legt den Benutzer an oder aktualisiert seinen Namen
  upsert(user: BoardUser): Promise<void>;
  byId(id: string): Promise<BoardUser | null>;
  list(): Promise<BoardUser[]>;
}

export interface ProjectRepository {
  list(): Promise<Project[]>;
  byId(id: number): Promise<Project | null>;
  byKey(key: string): Promise<Project | null>;
  insert(project: Omit<Project, 'id' | 'nextTicketNumber' | 'nextQuestionNumber'>): Promise<Project>;
  rename(id: number, name: string): Promise<void>;
  // Liefert die nächste freie Ticketnummer und zählt atomar weiter
  takeTicketNumber(id: number): Promise<number>;
  // Liefert die nächste freie Fragenummer und zählt atomar weiter
  takeQuestionNumber(id: number): Promise<number>;
}

export interface BoardFilter {
  projectId?: number;
  ownerId?: string;
}

export interface BoardRepository {
  list(filter?: BoardFilter): Promise<Board[]>;
  byId(id: number): Promise<Board | null>;
  insert(board: Omit<Board, 'id'>): Promise<Board>;
  update(board: Board): Promise<void>;
  delete(id: number): Promise<void>;
}

export interface TicketFilter {
  boardIds?: number[];
  ids?: string[];
  // Ohne die langen Texte (Beschreibung, Akzeptanz, Plan, Handbuch, Diagramm): ein leerer Text bleibt leer, ein
  // anderer darf durch ein Kennzeichen ersetzt kommen. Für Abhängigkeiten, Graphen und Board-Karten, die nur IDs,
  // Status, Branch und „Text vorhanden“ brauchen; solche Tickets nie zurückschreiben
  withoutTexts?: boolean;
}

export interface TicketRepository {
  // forUpdate sperrt die Zeile bis zum Ende der Transaktion (SQL: UPDLOCK)
  byId(id: string, forUpdate?: boolean): Promise<Ticket | null>;
  list(filter?: TicketFilter): Promise<Ticket[]>;
  insert(ticket: Ticket): Promise<void>;
  // Schreibt das Ticket; schlägt mit HarnessError „conflict“ fehl, wenn in der DB nicht rev-1 steht
  update(ticket: Ticket): Promise<void>;
  delete(id: string): Promise<void>;
  // IDs der Tickets, die dieses Ticket voraussetzen
  dependents(id: string): Promise<string[]>;
  // Entfernt alle Verweise anderer Tickets auf dieses Ticket und liefert deren IDs
  removeDependencyOn(id: string): Promise<string[]>;
  // Höchste SortOrder in einer Spalte eines Boards, null wenn leer
  maxSortOrder(boardId: number, status: TicketStatus): Promise<number | null>;
  // Zahl der Tickets je Board, die weder erledigt noch „nicht mehr nötig“ sind (Boards ohne solche fehlen)
  openCountByBoard(boardIds: number[]): Promise<Record<number, number>>;
}

// Neuer Kommentar; ohne Lesemarke gilt er als noch nicht eingespeist
export type NewComment = Omit<Comment, 'id' | 'readByAgentAt'> & { readByAgentAt?: string | null };

export interface CommentRepository {
  list(ticketId: string): Promise<Comment[]>;
  insert(comment: NewComment): Promise<Comment>;
  // Zahl der Kommentare je Ticket eines Boards (Tickets ohne Kommentar fehlen)
  countByBoard(boardId: number): Promise<Record<string, number>>;
  // Kommentare dieser Tickets ohne Lesemarke, in Reihenfolge des Anlegens
  unreadByAgent(ticketIds: string[]): Promise<Comment[]>;
  // Setzt die Lesemarke der Kommentare (bereits gesetzte bleiben unverändert)
  markReadByAgent(ids: number[], at: string): Promise<void>;
}

export interface TicketStepRepository {
  // Schritte eines Tickets in ihrer Reihenfolge
  list(ticketId: string): Promise<TicketStep[]>;
  // Ersetzt alle Schritte eines Tickets (leere Liste entfernt sie)
  replace(ticketId: string, steps: TicketStep[]): Promise<void>;
  // Setzt den Haken eines Schritts (Position ab 0); false, wenn es ihn nicht gibt
  setDone(ticketId: string, position: number, done: boolean): Promise<boolean>;
  // Erledigte und alle Schritte je Ticket; Tickets ohne Schritte fehlen
  progress(ticketIds: string[]): Promise<Record<string, StepProgress>>;
}

export interface TicketSessionRepository {
  // Verknüpft eine Session mit einem Ticket; false, wenn sie es schon war
  add(ticketId: string, sessionId: string, at: string): Promise<boolean>;
  // Sessions je Ticket in Reihenfolge der Verknüpfung; Tickets ohne Session fehlen
  byTickets(ticketIds: string[]): Promise<Record<string, string[]>>;
  // Tickets, mit denen die Session verknüpft ist (sortiert)
  ticketsOf(sessionId: string): Promise<string[]>;
}

export interface HistoryRepository {
  list(ticketId: string): Promise<HistoryEntry[]>;
  insert(entries: HistoryEntry[]): Promise<void>;
}

export interface NoteFilter {
  boardId: number;
  states?: NoteState[];
  source?: string;
}

export interface NoteRepository {
  list(filter: NoteFilter): Promise<Note[]>;
  // forUpdate sperrt die Zeile bis zum Ende der Transaktion (SQL: UPDLOCK)
  byId(id: number, forUpdate?: boolean): Promise<Note | null>;
  insert(note: Omit<Note, 'id'>): Promise<Note>;
  update(note: Note): Promise<void>;
}

export interface DeletionRepository {
  insert(deletion: Deletion): Promise<void>;
  list(): Promise<Deletion[]>;
}

// Auswahl von Fragen; genannte Kriterien werden mit „und“ verknüpft, leere Listen ergeben keine Fragen
export interface BoardQuestionFilter {
  projectId?: number;
  ids?: string[];
  // Fragen, die mindestens eines dieser Tickets betreffen
  ticketIds?: string[];
  states?: QuestionState[];
}

export interface BoardQuestionRepository {
  // forUpdate sperrt die Zeile bis zum Ende der Transaktion (SQL: UPDLOCK)
  byId(id: string, forUpdate?: boolean): Promise<BoardQuestion | null>;
  // Sortiert nach Projekt und Nummer
  list(filter?: BoardQuestionFilter): Promise<BoardQuestion[]>;
  insert(question: BoardQuestion): Promise<void>;
  // Schreibt die Frage samt Tags, Tickets und Abhängigkeiten; „conflict“, wenn in der DB nicht rev-1 steht
  update(question: BoardQuestion): Promise<void>;
  // IDs der Fragen, die das Ticket betreffen oder davon abhängen (sortiert); nur lesend
  idsReferencingTicket(ticketId: string): Promise<string[]>;
  // Entfernt alle Bezüge (betroffen, Abhängigkeit) auf ein Ticket; liefert die IDs der betroffenen Fragen (sortiert), Rev bleibt
  removeTicket(ticketId: string): Promise<string[]>;
}

export interface BoardQuestionResearchRepository {
  // Recherchen in Reihenfolge des Anlegens, Optionen nach Nummer
  list(questionId: string): Promise<BoardQuestionResearch[]>;
  insert(research: Omit<BoardQuestionResearch, 'id'>): Promise<BoardQuestionResearch>;
  // Zahl der Recherchen und Optionen der neuesten je Frage (Fragen ohne Recherche fehlen)
  summaries(questionIds: string[]): Promise<Record<string, BoardQuestionResearchSummary>>;
}

export interface BoardQuestionCommentRepository {
  list(questionId: string): Promise<BoardQuestionComment[]>;
  insert(comment: Omit<BoardQuestionComment, 'id'>): Promise<BoardQuestionComment>;
}

export interface BoardQuestionHistoryRepository {
  list(questionId: string): Promise<BoardQuestionHistoryEntry[]>;
  insert(entries: BoardQuestionHistoryEntry[]): Promise<void>;
}

// Suche über alle Projekte und Boards; nur lesend
export interface BoardSearchRepository {
  // Felder von Tickets, Kommentaren, Fragen, Fragen-Kommentaren und Notizen, in denen alle Begriffe vorkommen
  // (Groß/Klein egal, Teilwörter zählen); je Quelle (Tabelle und Feld) höchstens perSource Zeilen, neueste zuerst
  find(request: SearchRequest): Promise<SearchRows>;
}

export interface DispatchRunRepository {
  // Legt den Lauf an; false, wenn es die ID schon gibt (doppelte Erfassung)
  insert(run: DispatchRun): Promise<boolean>;
  // Läufe mit Start im Zeitraum, ältester zuerst
  list(filter: DispatchRunFilter): Promise<DispatchRun[]>;
  // Läufe aller Projekte mit Start in [from, to) (ISO, UTC), ältester zuerst
  listAll(from: string, to: string): Promise<DispatchRun[]>;
  // Frühester Start eines Laufs dieser Herkunft im Projekt; null ohne Lauf
  firstStarted(projectId: number, source: DispatchRunSource): Promise<string | null>;
  // IDs der vorhandenen Läufe unter diesen IDs (für idempotentes Nachtragen)
  existing(ids: string[]): Promise<string[]>;
  // Legt die Konfig-Version an, wenn es den Hash noch nicht gibt; false, wenn schon vorhanden
  recordConfigVersion(version: DispatchConfigVersion): Promise<boolean>;
  // Versionen, die in [from, to) zum ersten Mal auftraten, älteste zuerst
  listConfigVersions(from: string, to: string): Promise<DispatchConfigVersion[]>;
}

export interface DispatchStatsRepository {
  // Tickets des Projekts (optional Board), deren letzter Wechsel nach Erledigt in [from, to) liegt (ISO, UTC)
  doneTicketIds(filter: DispatchRunFilter): Promise<string[]>;
  // Statuswechsel und Blockiert-Gründe (Felder status, blockedReason) dieser Tickets, in Reihenfolge des Eintrags
  statusHistory(ticketIds: string[]): Promise<HistoryEntry[]>;
  // Zahl der Kommentare „[dispatch] Start…“ und „[dispatch] Abbruch…“ je Ticket (Tickets ohne fehlen)
  dispatchCommentCounts(ticketIds: string[]): Promise<Record<string, DispatchCommentCounts>>;
  // Kommentare „[dispatch] …“ der Tickets des Projekts, nach ID
  dispatchComments(projectId: number): Promise<Comment[]>;
}

// Alle Repositories innerhalb einer Transaktion
export interface BoardUnitOfWork {
  users: UserRepository;
  projects: ProjectRepository;
  boards: BoardRepository;
  tickets: TicketRepository;
  comments: CommentRepository;
  steps: TicketStepRepository;
  ticketSessions: TicketSessionRepository;
  history: HistoryRepository;
  notes: NoteRepository;
  deletions: DeletionRepository;
  questions: BoardQuestionRepository;
  questionResearch: BoardQuestionResearchRepository;
  questionComments: BoardQuestionCommentRepository;
  questionHistory: BoardQuestionHistoryRepository;
  search: BoardSearchRepository;
  dispatchRuns: DispatchRunRepository;
  dispatchStats: DispatchStatsRepository;
}

// Zugang zur Board-Datenbank: jede fachliche Aktion läuft in genau einer Transaktion
export interface BoardDatabase {
  transaction<T>(work: (uow: BoardUnitOfWork) => Promise<T>): Promise<T>;
  // Kennwert für den Stand eines Boards (Tickets, Kommentare, Notizen, Board, Fragen zu seinen Tickets)
  changeMarker(boardId: number): Promise<string>;
  // Kennwert für den Stand der Fragen eines Projekts; ändert sich bei jeder Änderung einer seiner Fragen
  questionMarker(projectId: number): Promise<string>;
  close(): Promise<void>;
}
