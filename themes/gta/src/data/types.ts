// Board-Stand, wie das Theme ihn braucht: nur die Felder, aus denen Spielwerte und Ereignisse entstehen.
// Der Server liefert mehr; parseBoardView nimmt seine Antwort tolerant an und lässt den Rest weg.

export type ColumnId = 'backlog' | 'waiting' | 'in_planning' | 'planned' | 'in_progress' | 'blocked' | 'done' | 'not_needed';

export interface Card {
  id: string;
  title: string;
  column: ColumnId;
  blockedReason: string | null;
  stepsDone: number;
  stepsTotal: number;
  priority: string;
  assignee: string | null;
}

export interface Column {
  id: ColumnId;
  title: string;
  tickets: Card[];
}

export interface BoardView {
  boardId: number;
  columns: Column[];
}

const COLUMN_IDS: readonly string[] = ['backlog', 'waiting', 'in_planning', 'planned', 'in_progress', 'blocked', 'done', 'not_needed'];

const isColumnId = (value: unknown): value is ColumnId => typeof value === 'string' && COLUMN_IDS.includes(value);
const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value);
const count = (value: unknown) => (typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : 0);
const textOrNull = (value: unknown) => (typeof value === 'string' ? value : null);

// Karte ohne ID oder Titel ist kaputt; fehlt die Spalte, gilt die der Spalte, in der sie steht
function parseCard(json: unknown, column: ColumnId): Card | null {
  if (!isRecord(json) || typeof json.id !== 'string' || typeof json.title !== 'string') return null;
  return {
    id: json.id,
    title: json.title,
    column: isColumnId(json.column) ? json.column : column,
    blockedReason: textOrNull(json.blockedReason),
    stepsDone: count(json.stepsDone),
    stepsTotal: count(json.stepsTotal),
    priority: typeof json.priority === 'string' ? json.priority : 'normal',
    assignee: textOrNull(json.assignee),
  };
}

function parseColumn(json: unknown): Column | null {
  if (!isRecord(json) || !isColumnId(json.id)) return null;
  const id = json.id;
  const tickets = Array.isArray(json.tickets) ? json.tickets.map((t) => parseCard(t, id)).filter((t): t is Card => t !== null) : [];
  return { id, title: typeof json.title === 'string' ? json.title : id, tickets };
}

/** Nimmt die Antwort des Servers tolerant an; unbekannte Spalten und kaputte Karten fallen weg, bei Unsinn null */
export function parseBoardView(json: unknown): BoardView | null {
  if (!isRecord(json) || typeof json.boardId !== 'number' || !Array.isArray(json.columns)) return null;
  return { boardId: json.boardId, columns: json.columns.map(parseColumn).filter((c): c is Column => c !== null) };
}
