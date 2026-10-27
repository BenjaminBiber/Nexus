import type { BoardActor, BoardServices } from '@harness/core';

// Was der Server von einer Board-Laufzeit braucht: BoardRuntime aus @harness/db oder eine In-Memory-Laufzeit in Tests.
export interface BoardBackend {
  readonly services: BoardServices;
  actor(name?: string): BoardActor;
  close(): Promise<void>;
}

// Baut eine Laufzeit auf (verbindet, prüft das Schema, legt den Benutzer an)
export type BoardBackendFactory = () => Promise<BoardBackend>;
