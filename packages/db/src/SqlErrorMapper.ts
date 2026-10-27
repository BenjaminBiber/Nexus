import { HarnessError } from '@harness/core';

// SQL-Server-Fehlernummer, wenn die Transaktion als Deadlock-Opfer abgebrochen wurde
export const SQL_DEADLOCK_VICTIM = 1205;

// SQL-Server-Fehlernummer, wenn dem Server der Arbeitsspeicher ausgeht (z. B. Container mit zu wenig RAM)
export const SQL_OUT_OF_MEMORY = 701;

// SQL-Server-Fehlernummern für doppelte Schlüssel (Primärschlüssel bzw. eindeutiger Index)
export const SQL_UNIQUE_VIOLATIONS = [2627, 2601] as const;

// node:sqlite: Fehler tragen code 'ERR_SQLITE_ERROR' und den erweiterten Ergebniscode in errcode
const SQLITE_ERROR_CODE = 'ERR_SQLITE_ERROR';
// Primärcodes (errcode & 0xff) von SQLite
export const SQLITE_BUSY = 5;
export const SQLITE_LOCKED = 6;
export const SQLITE_IOERR = 10;
export const SQLITE_FULL = 13;
// Erweiterte Codes für doppelte Schlüssel
export const SQLITE_CONSTRAINT_PRIMARYKEY = 1555;
export const SQLITE_CONSTRAINT_UNIQUE = 2067;

// Übersetzt bekannte SQL-Server- und node:sqlite-Fehler in erwartbare Harness-Fehler; alles andere bleibt unverändert.
export class SqlErrorMapper {
  // Deadlock-Opfer bzw. gesperrte Datei wird zu 'conflict' (erneut versuchen), Speichermangel zu
  // 'board-db-overloaded', volle Platte bzw. E/A-Fehler zu 'board-db-storage'; sonst der Originalfehler
  static map(error: unknown): unknown {
    const number = SqlErrorMapper.numberOf(error);
    if (number === SQL_DEADLOCK_VICTIM) {
      return new HarnessError('conflict', 'Gleichzeitige Änderung – bitte erneut versuchen.');
    }
    if (number === SQL_OUT_OF_MEMORY) {
      return new HarnessError('board-db-overloaded', 'Der Board-Datenbank geht der Arbeitsspeicher aus – bitte kurz warten und erneut versuchen (ggf. Docker/WSL-Speicher freigeben oder das Speicherlimit des Containers prüfen).');
    }
    const sqlite = SqlErrorMapper.sqliteCodeOf(error);
    if (sqlite !== undefined) {
      const primary = sqlite & 0xff;
      // Datei von einem anderen Prozess gesperrt: wie ein Deadlock erneut versuchen
      if (primary === SQLITE_BUSY || primary === SQLITE_LOCKED) {
        return new HarnessError('conflict', 'Gleichzeitige Änderung – bitte erneut versuchen.');
      }
      if (primary === SQLITE_FULL) {
        return new HarnessError('board-db-storage', 'Die Board-Datei kann nicht wachsen: Der Datenträger ist voll – bitte Speicherplatz freigeben und erneut versuchen.');
      }
      if (primary === SQLITE_IOERR) {
        const detail = (error as { errstr?: unknown }).errstr;
        return new HarnessError('board-db-storage', `Lese- oder Schreibfehler an der Board-Datei${typeof detail === 'string' ? ` (${detail})` : ''} – bitte Datenträger und Zugriffsrechte prüfen.`);
      }
    }
    return error;
  }

  // Roher node:sqlite-Fehler „Datei gesperrt“ (SQLITE_BUSY samt erweiterter Codes wie BUSY_SNAPSHOT), noch nicht übersetzt
  static isSqliteBusy(error: unknown): boolean {
    const sqlite = SqlErrorMapper.sqliteCodeOf(error);
    return sqlite !== undefined && (sqlite & 0xff) === SQLITE_BUSY;
  }

  // Doppelter Schlüssel: SQL Server 2627/2601 oder SQLite UNIQUE/PRIMARY KEY (2067/1555)
  static isUniqueViolation(error: unknown): boolean {
    const number = SqlErrorMapper.numberOf(error);
    if (number !== undefined && (SQL_UNIQUE_VIOLATIONS as readonly number[]).includes(number)) return true;
    const sqlite = SqlErrorMapper.sqliteCodeOf(error);
    return sqlite === SQLITE_CONSTRAINT_UNIQUE || sqlite === SQLITE_CONSTRAINT_PRIMARYKEY;
  }

  // Fehlernummer direkt am Fehler oder am ursprünglichen Treiberfehler (tedious)
  private static numberOf(error: unknown): number | undefined {
    if (!error || typeof error !== 'object') return undefined;
    const e = error as { number?: unknown; originalError?: { number?: unknown; info?: { number?: unknown } } };
    for (const candidate of [e.number, e.originalError?.number, e.originalError?.info?.number]) {
      if (typeof candidate === 'number') return candidate;
    }
    return undefined;
  }

  // Erweiterter SQLite-Ergebniscode eines node:sqlite-Fehlers; undefined bei anderen Fehlern
  private static sqliteCodeOf(error: unknown): number | undefined {
    if (!error || typeof error !== 'object') return undefined;
    const e = error as { code?: unknown; errcode?: unknown };
    return e.code === SQLITE_ERROR_CODE && typeof e.errcode === 'number' ? e.errcode : undefined;
  }
}
