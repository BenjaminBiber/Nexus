import { HarnessError } from '@harness/core';
import { SqlErrorMapper } from '@harness/db';
import type { BoardBackend, BoardBackendFactory } from './BoardBackend.js';

// Fehlercodes des Treibers, nach denen die Verbindung als verloren gilt (ETIMEOUT bewusst nicht: mssql nutzt ihn auch für Abfrage-Timeouts)
const CONNECTION_CODES = new Set(['ESOCKET', 'ECONNCLOSED', 'ECONNRESET', 'ECONNREFUSED', 'ENOTOPEN']);

// SQLite-Primärcodes (errcode & 0xff), nach denen die Board-Datei als nicht erreichbar gilt:
// SQLITE_PERM 3, SQLITE_READONLY 8, SQLITE_CANTOPEN 14, SQLITE_NOTADB 26.
// BUSY/LOCKED (Konflikt) sowie IOERR/FULL (board-db-storage) übersetzt SqlErrorMapper.
const SQLITE_FILE_CODES = new Set([3, 8, 14, 26]);
// Dateisystemfehler beim Anlegen des Ordners der Board-Datei
const FOLDER_CODES = new Set(['EACCES', 'EPERM', 'EROFS', 'ENOENT']);

// Hält genau eine Board-Laufzeit für den Server: Aufbau beim ersten Bedarf, Neuaufbau nach Verbindungsfehler oder Konfig-Änderung.
export class BoardRuntimeHolder {
  private pending: Promise<BoardBackend> | null = null;

  constructor(private readonly factory: BoardBackendFactory) {}

  // Übersetzt eine kaputte board.yaml in einen Fehler, den der Server als 503 meldet
  static translate(error: unknown): unknown {
    if (error instanceof HarnessError && error.code === 'board-config-invalid') {
      return new HarnessError('board-config-broken', `${error.message} Unter Einstellungen → Board-Datenbank reparieren.`);
    }
    if (BoardRuntimeHolder.isSqliteFileError(error)) {
      return new HarnessError('board-db-unreachable', `Board-Datei ist nicht erreichbar: ${(error as Error).message}`);
    }
    if (error && typeof error === 'object') {
      const e = error as { syscall?: unknown; code?: unknown; message?: unknown };
      if (e.syscall === 'mkdir' && typeof e.code === 'string' && FOLDER_CODES.has(e.code)) {
        return new HarnessError('board-db-unreachable', `Ordner der Board-Datei lässt sich nicht anlegen: ${e.message}`);
      }
    }
    return error;
  }

  // Laufzeit; gleichzeitige Aufrufe teilen sich den Aufbau, ein Fehlschlag wird nicht gemerkt
  get(): Promise<BoardBackend> {
    if (!this.pending) {
      const started: Promise<BoardBackend> = this.factory().catch((error: unknown) => {
        if (this.pending === started) this.pending = null;
        throw BoardRuntimeHolder.translate(error);
      });
      this.pending = started;
    }
    return this.pending;
  }

  // Führt Arbeit mit der Laufzeit aus; ein Verbindungsfehler verwirft sie und wird zu board-db-unreachable
  async run<T>(work: (backend: BoardBackend) => Promise<T>): Promise<T> {
    const started = this.get();
    const backend = await started;
    try {
      return await work(backend);
    } catch (error) {
      if (BoardRuntimeHolder.isConnectionError(error)) {
        await this.discard(started);
        throw new HarnessError('board-db-unreachable', `Verbindung zur Board-Datenbank verloren: ${(error as Error).message}`);
      }
      // Lesezugriffe laufen außerhalb von Transaktionen und werden dort nicht übersetzt (z. B. Speichermangel)
      throw SqlErrorMapper.map(error);
    }
  }

  // Verwirft nur die fehlerhafte Laufzeit; ist inzwischen eine neue gebaut, bleibt sie unberührt
  private async discard(failed: Promise<BoardBackend>): Promise<void> {
    if (this.pending === failed) await this.reset();
  }

  // Verwirft die Laufzeit (nach geänderter Konfig oder beim Beenden); die nächste Anfrage baut neu auf
  async reset(): Promise<void> {
    const current = this.pending;
    this.pending = null;
    if (!current) return;
    try {
      await (await current).close();
    } catch {
      // Aufbau oder Schließen war ohnehin gescheitert
    }
  }

  // Netz- oder Verbindungsfehler des Treibers (mssql ConnectionError/Socket-Codes, SQLite-Datei-/Zugriffsfehler oder geschlossene SQLite-Verbindung)
  static isConnectionError(error: unknown): boolean {
    if (!error || typeof error !== 'object' || error instanceof HarnessError) return false;
    const e = error as { code?: unknown; message?: unknown };
    if (BoardRuntimeHolder.isMssqlConnectionError(error) || BoardRuntimeHolder.isSqliteFileError(error)) return true;
    return e.code === 'ERR_INVALID_STATE' && typeof e.message === 'string' && /database is not open/i.test(e.message);
  }

  // mssql: ConnectionError oder einer der Socket-Codes
  private static isMssqlConnectionError(error: unknown): boolean {
    if (!error || typeof error !== 'object') return false;
    const e = error as { name?: unknown; code?: unknown };
    return e.name === 'ConnectionError' || (typeof e.code === 'string' && CONNECTION_CODES.has(e.code));
  }

  // node:sqlite: Datei-/Zugriffsfehler (nicht BUSY/LOCKED, nicht IOERR/FULL – die übersetzt SqlErrorMapper)
  private static isSqliteFileError(error: unknown): boolean {
    if (!error || typeof error !== 'object') return false;
    const e = error as { code?: unknown; errcode?: unknown };
    return e.code === 'ERR_SQLITE_ERROR' && typeof e.errcode === 'number' && SQLITE_FILE_CODES.has(e.errcode & 0xff);
  }

  // Verbindungsfehler im weiteren Sinn: Treiberfehler oder bereits übersetztes board-db-unreachable; alles andere (auch TypeError) ist ein Programm- oder Fachfehler
  static isConnectionFailure(error: unknown): boolean {
    return BoardRuntimeHolder.isConnectionError(error) || (error instanceof HarnessError && error.code === 'board-db-unreachable');
  }
}
