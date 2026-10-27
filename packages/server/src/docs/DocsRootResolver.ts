import { existsSync, statSync } from 'node:fs';
import { BoardConfigStore, HarnessError, type Board, type HarnessHome } from '@harness/core';
import type { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';

// Warum es keine Docs-Quelle gibt: kein Board zum Ordner, Board ohne Repo-Pfad, Pfad zeigt ins Leere
export type DocsSourceReason = 'no-board' | 'no-repo-path' | 'missing-folder';

// Woher die Dokumente kommen; ohne Quelle sagt `reason`, was fehlt – die Docs-Seite zeigt dann einen Hinweis.
export interface DocsSource {
  /** Absoluter Repo-Pfad des Boards mit / als Trenner, sonst null */
  root: string | null;
  boardId: number | null;
  boardName: string | null;
  /** nur gesetzt, wenn root === null */
  reason: DocsSourceReason | null;
}

const NO_BOARD: DocsSource = { root: null, boardId: null, boardName: null, reason: 'no-board' };

// Findet zum Ordner eines Projekts das Board und damit den Repo-Pfad, aus dem die Docs gelesen werden.
export class DocsRootResolver {
  constructor(
    private readonly holder: BoardRuntimeHolder | null,
    private readonly home: HarnessHome | null,
  ) {}

  // Nie werfend: ohne Board-Laufzeit, ohne board.yaml oder bei Verbindungsfehler gibt es eben keine Quelle
  async resolve(projectRoot: string): Promise<DocsSource> {
    if (!this.holder || !this.home) return NO_BOARD;
    const config = await new BoardConfigStore(this.home).read().catch(() => null);
    if (!config) return NO_BOARD;
    let board: Board | null;
    try {
      board = await this.holder.run(async (b) => {
        const found = await b.services.boards.forRepo(projectRoot, { userId: config.userId, activeBoard: config.activeBoard });
        if (found || config.activeBoard === null) return found;
        // Ohne Treffer erklärt nur ein aktives Board ohne Repo-Pfad die fehlende Quelle; zeigt es auf ein
        // anderes Repo, gehört es nicht zu diesem Ordner und die Docs bleiben leer.
        const active = await b.services.boards.find(config.activeBoard);
        return active && !active.repoPath?.trim() ? active : null;
      });
    } catch (error) {
      if (error instanceof HarnessError) return NO_BOARD;
      throw error;
    }
    return sourceOf(board);
  }

  // Quelle einer Anfrage: mit `board` aus der Query (Board-Umschalter der Docs-Seite) dieses Board, sonst das Board des Projektordners
  async resolveFor(projectRoot: string, board: unknown): Promise<DocsSource> {
    const id = typeof board === 'string' && /^\d+$/.test(board) ? Number(board) : 0;
    return id > 0 ? this.forBoard(id) : this.resolve(projectRoot);
  }

  // Quelle eines bestimmten Boards (Board-Umschalter der Docs-Seite); unbekanntes Board oder fehlende Laufzeit heißt „kein Board“
  async forBoard(boardId: number): Promise<DocsSource> {
    if (!this.holder) return NO_BOARD;
    try {
      return sourceOf(await this.holder.run((b) => b.services.boards.find(boardId)));
    } catch (error) {
      if (error instanceof HarnessError) return NO_BOARD;
      throw error;
    }
  }
}

// Quelle eines Boards: sein Repo-Pfad, wenn er auf einen Ordner zeigt, sonst der Grund, warum es keine gibt
function sourceOf(board: Board | null): DocsSource {
  if (!board) return NO_BOARD;
  const path = board.repoPath?.trim() ?? '';
  if (!path) return { root: null, boardId: board.id, boardName: board.name, reason: 'no-repo-path' };
  if (!existsSync(path) || !statSync(path).isDirectory()) {
    return { root: null, boardId: board.id, boardName: board.name, reason: 'missing-folder' };
  }
  return { root: path.replace(/\\/g, '/'), boardId: board.id, boardName: board.name, reason: null };
}
