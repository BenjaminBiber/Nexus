import { HarnessError, type BoardConfig } from '@harness/core';
import type { ConnectOptions, ManagedBoardDatabase } from './ManagedBoardDatabase.js';
import { MssqlBoardDatabase } from './MssqlBoardDatabase.js';
import { SqliteBoardDatabase } from './sqlite/SqliteBoardDatabase.js';

// Öffnet die Board-Datenbank des in der Konfig gewählten Anbieters.
export class BoardDatabaseFactory {
  // mssql: Verbindung zum SQL Server; sqlite: lokale Datei aus config.file (legt Datei und Schema selbst an)
  static async open(config: BoardConfig, options: ConnectOptions = {}): Promise<ManagedBoardDatabase> {
    const provider: unknown = config.provider;
    switch (provider) {
      case 'mssql':
        return MssqlBoardDatabase.connect(config, options);
      case 'sqlite':
        // options (Verbindungs-Timeout) gilt nur für MSSQL; eine lokale Datei wird ohne Zeitlimit geöffnet.
        return SqliteBoardDatabase.open(config.file);
      default:
        throw new HarnessError(
          'board-provider-unknown',
          `Unbekannter Anbieter der Board-Datenbank „${String(provider)}“ – erlaubt sind mssql und sqlite.`,
        );
    }
  }
}
