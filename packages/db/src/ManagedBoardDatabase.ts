import type { BoardDatabase } from '@harness/core';

/** Einstellungen für den Verbindungsaufbau (gelten für jeden Anbieter; SQLite ignoriert das Zeitlimit) */
export interface ConnectOptions {
  /** Zeitlimit für den Verbindungsaufbau in ms (Standard 8000) */
  connectionTimeoutMs?: number;
}

// Board-Datenbank eines Anbieters (SQL Server, SQLite), wie BoardRuntime sie braucht:
// Board-Zugriff plus Schema-Stand für den SchemaVersionGuard; close() kommt aus BoardDatabase.
export interface ManagedBoardDatabase extends BoardDatabase {
  // Höchste eingespielte Schema-Version; null, wenn das Schema fehlt
  schemaVersion(): Promise<number | null>;
  // Eingespielte additive Erweiterungen (z. B. 'dispatch-runs'); leer, wenn es keine gibt
  schemaFeatures(): Promise<string[]>;
  // Spielt fehlende additive Erweiterungen selbst ein, soweit Rechte und Anbieter das zulassen; liefert die eingespielten Namen
  ensureSchemaFeatures(missing: string[]): Promise<string[]>;
  // Merkt die Erweiterungen, die danach immer noch fehlen; nur die Zugriffe darauf verweigern sich dann
  setMissingFeatures(names: string[]): void;
}
