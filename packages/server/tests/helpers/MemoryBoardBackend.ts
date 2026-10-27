import { BoardRepoPaths, BoardServices, InMemoryBoardDatabase, MemoryBoardRepoStore, type BoardActor, type BoardConfig } from '@harness/core';
import type { BoardBackend } from '../../src/dbboard/BoardBackend.js';

export const MEMORY_USER = { id: '11111111-1111-1111-1111-111111111111', name: 'Florian' };

// Platzhalter-Konfig für Tests (kein echter Server, kein echtes Passwort)
export const MEMORY_CONFIG: BoardConfig = {
  // file wird nur bei provider sqlite genutzt
  provider: 'mssql', file: '',
  server: 'memory', port: 1433, database: 'AiHarness_Test', user: 'test', password: 'test',
  encrypt: false, trustServerCertificate: true, userId: MEMORY_USER.id, userName: MEMORY_USER.name, activeBoard: null,
};

// Board-Laufzeit im Speicher für Servertests: gleiche Dienste wie BoardRuntime, ohne SQL Server.
export class MemoryBoardBackend implements BoardBackend {
  readonly db = new InMemoryBoardDatabase();
  // Eigene Repo-Pfade des Testbenutzers im Speicher
  readonly repoStore = new MemoryBoardRepoStore();
  readonly services = new BoardServices(this.db, undefined, new BoardRepoPaths(this.repoStore, MEMORY_USER.id));
  closed = false;

  private constructor() {}

  // Legt die Laufzeit mit dem Testbenutzer an
  static async create(): Promise<MemoryBoardBackend> {
    const backend = new MemoryBoardBackend();
    await backend.db.transaction((u) => u.users.upsert(MEMORY_USER));
    return backend;
  }

  // Handelnder wie bei BoardRuntime: GUID des Testbenutzers, Name aus Angabe oder Anzeigename
  actor(name?: string): BoardActor {
    return { userId: MEMORY_USER.id, name: name?.trim() || MEMORY_USER.name };
  }

  // Merkt sich nur, dass geschlossen wurde
  async close(): Promise<void> {
    this.closed = true;
  }
}
