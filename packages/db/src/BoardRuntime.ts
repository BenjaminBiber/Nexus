import { BoardConfigStore, BoardRepoFile, BoardRepoPaths, BoardServices, HarnessHome, type BoardActor, type BoardConfig } from '@harness/core';
import { BoardDatabaseFactory } from './BoardDatabaseFactory.js';
import type { ConnectOptions, ManagedBoardDatabase } from './ManagedBoardDatabase.js';
import { SchemaVersionGuard } from './SchemaVersionGuard.js';

// Laufzeit des Boards: Konfig lesen, verbinden, Schema prüfen, Benutzer anlegen, Dienste bereitstellen.
export class BoardRuntime {
  private constructor(
    readonly config: BoardConfig,
    private readonly db: ManagedBoardDatabase,
    readonly services: BoardServices,
    readonly missingFeatures: string[] = [],
  ) {}

  // Öffnet das Board mit ~/.harness/board.yaml; eigene Repo-Pfade aus ~/.harness/board-repos.yaml
  static async open(home: HarnessHome, options: ConnectOptions = {}): Promise<BoardRuntime> {
    return BoardRuntime.connect(await new BoardConfigStore(home).require(), options, home);
  }

  // Verbindet mit einer gegebenen Konfig (auch für den Verbindungstest der Oberfläche)
  // open ist überschreibbar, damit Tests ohne echte Datenbank verbinden können
  static async connect(config: BoardConfig, options: ConnectOptions = {}, home: HarnessHome = new HarnessHome(), open = BoardDatabaseFactory.open): Promise<BoardRuntime> {
    const db: ManagedBoardDatabase = await open(config, options);
    let missing: string[] = [];
    try {
      missing = new SchemaVersionGuard().check(
        await db.schemaVersion(),
        config.provider === 'sqlite' ? config.file : config.database,
        await db.schemaFeatures(),
        config.provider,
      );
      // Fehlende Erweiterungen sind additiv: erst selbst einspielen, und was danach bleibt, sperrt nur den Zugriff darauf
      if (missing.length) {
        await db.ensureSchemaFeatures(missing);
        missing = SchemaVersionGuard.missing(await db.schemaFeatures());
        db.setMissingFeatures(missing);
      }
      await db.transaction((u) => u.users.upsert({ id: config.userId, name: config.userName }));
    } catch (error) {
      await db.close();
      throw error;
    }
    return new BoardRuntime(config, db, new BoardServices(db, undefined, new BoardRepoPaths(new BoardRepoFile(home), config.userId)), missing);
  }

  // Handelnder: eigene GUID, Name aus --by/--author/--agent oder der Anzeigename
  actor(name?: string): BoardActor {
    return { userId: this.config.userId, name: name?.trim() || this.config.userName };
  }

  // Schließt die Verbindung
  close(): Promise<void> {
    return this.db.close();
  }
}
