import { randomUUID } from 'node:crypto';
import { BoardConfigStore, HarnessError, type BoardConfig, type BoardProvider, type HarnessHome } from '@harness/core';
import { BoardRuntime } from '@harness/db';
import type { FastifyInstance } from 'fastify';

export interface BoardConfigDto {
  configured: boolean;
  // Anbieter der Board-Datenbank
  provider: BoardProvider;
  // Pfad der SQLite-Datei (bei mssql bzw. ohne Konfig der Vorschlag <HARNESS_HOME>/board.db)
  dbFile: string;
  // Pfad der board.yaml
  file: string;
  server: string;
  port: number;
  database: string;
  user: string;
  hasPassword: boolean;
  encrypt: boolean;
  trustServerCertificate: boolean;
  userId: string;
  userName: string;
  activeBoard: number | null;
}

// /api/board-config: Verbindung zur Board-Datenbank lesen, speichern und testen (Passwort nie im Klartext).
export class BoardConfigRoutes {
  private readonly store: BoardConfigStore;

  // onSaved: nach dem Speichern aufgerufen (der Server verwirft dann seine Board-Laufzeit)
  constructor(private readonly home: HarnessHome, private readonly onSaved: () => Promise<void> = async () => {}) {
    this.store = new BoardConfigStore(home);
  }

  // Meldet die Routen an
  register(app: FastifyInstance): void {
    app.get('/api/board-config', async () => this.dto(await this.store.read()));
    app.put('/api/board-config', async (request) => {
      const config = this.merge(await this.readForRepair(), (request.body ?? {}) as Record<string, unknown>);
      await this.store.write(config);
      await this.onSaved();
      return this.dto(config);
    });
    app.post('/api/board-config/test', async (request) => {
      try {
        const config = this.merge(await this.readForRepair(), (request.body ?? {}) as Record<string, unknown>);
        const runtime = await BoardRuntime.connect(config);
        await runtime.close();
        const message = config.provider === 'sqlite'
          ? `Datei geöffnet, Schema passt (${config.file}).`
          : 'Verbindung erfolgreich, Schema-Version passt.';
        return { ok: true, message };
      } catch (error) {
        if (error instanceof HarnessError) return { ok: false, message: error.message };
        return { ok: false, message: `Unerwarteter Fehler: ${(error as Error).message}` };
      }
    });
  }

  // Gespeicherte Konfig als Grundlage für Speichern/Test; eine kaputte Datei gilt als nicht vorhanden,
  // damit die Oberfläche sie reparieren kann (GET meldet den Fehler weiterhin)
  private async readForRepair(): Promise<BoardConfig | null> {
    try {
      return await this.store.read();
    } catch (error) {
      if (error instanceof HarnessError && error.code === 'board-config-invalid') return null;
      throw error;
    }
  }

  // Antwort ohne Passwort; ohne Datei Standardwerte mit neuer Vorschlags-GUID
  private dto(config: BoardConfig | null): BoardConfigDto {
    if (!config) {
      return {
        configured: false, provider: 'mssql', dbFile: this.home.boardDbFile, file: this.store.file,
        server: '', port: 1433, database: 'AiHarness', user: '', hasPassword: false,
        encrypt: true, trustServerCertificate: true, userId: randomUUID(), userName: '', activeBoard: null,
      };
    }
    // file im DTO bleibt der Pfad der board.yaml; der Pfad der SQLite-Datei heißt dbFile
    const { password, file: dbFile, ...rest } = config;
    return { configured: true, file: this.store.file, ...rest, dbFile, hasPassword: password.length > 0 };
  }

  // Übernimmt die Eingabe; leeres Passwort behält das gespeicherte, das aktive Board bleibt
  private merge(current: BoardConfig | null, body: Record<string, unknown>): BoardConfig {
    const password = typeof body.password === 'string' && body.password !== '' ? body.password : current?.password ?? '';
    // dbFile aus der Oberfläche wird zu file; ein mitgeschicktes file (Pfad der board.yaml) zählt nicht
    const { dbFile, file: _yamlFile, ...input } = body;
    const file = typeof dbFile === 'string' ? dbFile : current?.file;
    return this.store.validate({ ...current, ...input, file, password, activeBoard: current?.activeBoard ?? null });
  }
}
