// Gemerkter Zustand der Docs-Werkbank je Projekt: offene Knoten des Explorers und die Breiten der Seitenspalten.
// Wie bei docsSeen.ts ist ein gesperrter oder kaputter Speicher kein Fehler – die Seite fällt dann auf ihren Standard zurück.

const openKey = (pid: string) => `harness.docsTreeOpen.${pid}`;
const widthKey = (pid: string) => `harness.docsWidths.${pid}`;

// Breiten der beiden Seitenspalten in Pixeln
export interface DocsWidths {
  explorer: number;
  tools: number;
}

// Liest einen gespeicherten Wert als JSON; null bedeutet „kein brauchbarer Stand“
function readJson(key: string): unknown {
  try {
    const raw = window.localStorage.getItem(key);
    return raw === null ? null : JSON.parse(raw);
  } catch {
    return null;
  }
}

// Schreibt einen Wert als JSON; Fehler des Speichers werden bewusst ignoriert
function writeJson(key: string, value: unknown): void {
  try {
    window.localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* Speicher gesperrt: die Seite funktioniert auch ohne */
  }
}

// Liest die Knoten des Explorers, die vom Standard abweichen (zugeklappte Gruppen, aufgeklappte Themen); null, wenn nichts (Brauchbares) gespeichert ist
export function loadDocsTreeOpen(pid: string): string[] | null {
  const value = readJson(openKey(pid));
  if (!Array.isArray(value) || value.some((id) => typeof id !== 'string')) return null;
  return value as string[];
}

// Merkt die vom Standard abweichenden Knoten des Explorers
export function saveDocsTreeOpen(pid: string, ids: string[]): void {
  writeJson(openKey(pid), ids);
}

// Liest die gemerkten Spaltenbreiten; null, wenn nichts (Brauchbares) gespeichert ist
export function loadDocsWidths(pid: string): DocsWidths | null {
  const value = readJson(widthKey(pid));
  if (value === null || typeof value !== 'object') return null;
  const { explorer, tools } = value as Record<string, unknown>;
  if (!Number.isFinite(explorer) || !Number.isFinite(tools)) return null;
  return { explorer: explorer as number, tools: tools as number };
}

// Merkt die Spaltenbreiten
export function saveDocsWidths(pid: string, widths: DocsWidths): void {
  writeJson(widthKey(pid), widths);
}

const boardsKey = (pid: string) => `harness.docsBoards.${pid}`;

// Auswahl des Board-Umschalters: „all“ für alle Boards mit Repo, sonst die gewählten Board-IDs
export type DocsBoardSelection = 'all' | number[];

// Liest die gemerkte Board-Auswahl; null, wenn nichts (Brauchbares) gespeichert ist – dann gilt das Board des Ordners
export function loadDocsBoards(pid: string): DocsBoardSelection | null {
  const value = readJson(boardsKey(pid));
  if (value === 'all') return 'all';
  if (!Array.isArray(value) || value.length === 0 || value.some((id) => !Number.isInteger(id))) return null;
  return value as number[];
}

// Merkt die Board-Auswahl; null vergisst sie (zurück zum Board des Ordners)
export function saveDocsBoards(pid: string, selection: DocsBoardSelection | null): void {
  if (selection === null) {
    try {
      window.localStorage.removeItem(boardsKey(pid));
    } catch {
      /* Speicher gesperrt: die Seite funktioniert auch ohne */
    }
    return;
  }
  writeJson(boardsKey(pid), selection);
}
