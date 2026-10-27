// Art der Board-Datenbank: gemeinsamer SQL Server oder lokale SQLite-Datei (Einzelplatz)
export type BoardProvider = 'mssql' | 'sqlite';
export const BOARD_PROVIDERS: readonly BoardProvider[] = ['mssql', 'sqlite'];

// Verbindung zur Board-Datenbank und Identität dieses Rechners (Datei ~/.harness/board.yaml).
export interface BoardConfig {
  // Anbieter der Board-Datenbank; fehlt er in board.yaml, gilt 'mssql'
  provider: BoardProvider;
  // Pfad der SQLite-Datei (nur bei provider 'sqlite' genutzt); Standard <HARNESS_HOME>/board.db
  file: string;
  server: string;
  port: number;
  database: string;
  user: string;
  password: string;
  encrypt: boolean;
  trustServerCertificate: boolean;
  // GUID des Entwicklers; ordnet Boards, Kommentare und Verlauf zu
  userId: string;
  userName: string;
  // Board, auf dem CLI-Befehle ohne --board arbeiten
  activeBoard: number | null;
}

export const GUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
