import type { BoardActor, BoardServices } from '@harness/core';

// Was Hooks von der Board-Laufzeit brauchen: BoardRuntime aus @harness/db oder eine In-Memory-Laufzeit in Tests.
export interface BoardAccess {
  readonly services: BoardServices;
  readonly config: { userId: string; activeBoard: number | null };
  actor(name?: string): BoardActor;
  close(): Promise<void>;
}

// Öffnet die Board-Laufzeit; null, wenn keine Konfig da oder die DB nicht erreichbar ist
export type BoardOpener = (env: Record<string, string | undefined>) => Promise<BoardAccess | null>;
