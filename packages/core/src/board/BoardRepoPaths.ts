import type { Board } from './BoardTypes.js';

// Herkunft des wirksamen Repo-Pfads: lokale Zuordnung dieses Benutzers oder DB-Altbestand (Board.RepoPath, nur eigene Boards)
export type BoardRepoSource = 'local' | 'legacy';

// Board mit dem wirksamen Repo-Pfad des Benutzers in `repoPath` und dessen Herkunft
export type WithRepoSource<T extends Board> = T & { repoSource: BoardRepoSource | null };

// Lokale Zuordnung Board → Repo-Pfad; null heißt „ausdrücklich kein Pfad“ (verdrängt den Altbestand)
export interface BoardRepoStore {
  read(): Promise<Map<number, string | null>>;
  write(entries: Map<number, string | null>): Promise<void>;
}

// Zuordnung im Speicher (Tests und Laufzeiten ohne Benutzerordner)
export class MemoryBoardRepoStore implements BoardRepoStore {
  private entries: Map<number, string | null>;

  constructor(entries: Map<number, string | null> = new Map()) {
    this.entries = new Map(entries);
  }

  async read(): Promise<Map<number, string | null>> {
    return new Map(this.entries);
  }

  async write(entries: Map<number, string | null>): Promise<void> {
    this.entries = new Map(entries);
  }
}

// Zentrale Stelle für den Repo-Pfad eines Boards: jeder Benutzer pflegt seinen Pfad lokal (board-repos.yaml).
// Wirksam ist der lokale Eintrag; fehlt er, gilt bei eigenen Boards der DB-Wert (Altbestand), bei fremden nie.
export class BoardRepoPaths {
  constructor(private readonly store: BoardRepoStore, private readonly userId: string | null) {}

  // Boards mit wirksamem Pfad in `repoPath` und dessen Herkunft
  async apply<T extends Board>(boards: T[]): Promise<Array<WithRepoSource<T>>> {
    if (!boards.length) return [];
    const entries = await this.store.read();
    return boards.map((b) => this.resolve(b, entries));
  }

  // Ein Board mit wirksamem Pfad
  async one<T extends Board>(board: T): Promise<WithRepoSource<T>> {
    return this.resolve(board, await this.store.read());
  }

  // Setzt den eigenen Pfad des Boards; leer oder null hinterlegt „kein Pfad“
  async set(boardId: number, path: string | null): Promise<void> {
    const entries = await this.store.read();
    entries.set(boardId, path && path.trim() ? path.trim() : null);
    await this.store.write(entries);
  }

  // Wirksamer Pfad eines Boards aus einer gelesenen Zuordnung
  private resolve<T extends Board>(board: T, entries: Map<number, string | null>): WithRepoSource<T> {
    if (entries.has(board.id)) {
      const path = entries.get(board.id) ?? null;
      return { ...board, repoPath: path, repoSource: path ? 'local' : null };
    }
    if (this.userId !== null && board.ownerId === this.userId && board.repoPath) return { ...board, repoSource: 'legacy' };
    return { ...board, repoPath: null, repoSource: null };
  }
}
