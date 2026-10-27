import { homedir } from 'node:os';
import { join } from 'node:path';

// Benutzerweiter Ordner des Harness (`~/.harness`, überschreibbar mit HARNESS_HOME).
export class HarnessHome {
  readonly dir: string;

  constructor(env: Record<string, string | undefined> = process.env) {
    this.dir = env.HARNESS_HOME || join(homedir(), '.harness');
  }

  get projectsFile(): string { return join(this.dir, 'projects.json'); }
  get projectsLock(): string { return join(this.dir, 'projects.lock'); }
  get serverFile(): string { return join(this.dir, 'server.json'); }
  get logFile(): string { return join(this.dir, 'server.log'); }
  get sessionsFile(): string { return join(this.dir, 'sessions.json'); }
  get favoritesFile(): string { return join(this.dir, 'favorites.json'); }
  get settingsFile(): string { return join(this.dir, 'settings.json'); }
  get backupsDir(): string { return join(this.dir, 'backups'); }
  get boardConfigFile(): string { return join(this.dir, 'board.yaml'); }
  /** Standardpfad der SQLite-Datei des Boards (provider: sqlite ohne eigenes file) */
  get boardDbFile(): string { return join(this.dir, 'board.db'); }
  get dispatchConfigFile(): string { return join(this.dir, 'dispatch.yaml'); }
  /** Ausblendungen und letzte Claude-Analyse der Optimierungs-Vorschläge */
  get dispatchSuggestionsFile(): string { return join(this.dir, 'dispatch-suggestions.json'); }
  /** Eigene Repo-Pfade der Boards (je Benutzer lokal, nicht in der gemeinsamen DB) */
  get boardReposFile(): string { return join(this.dir, 'board-repos.yaml'); }
  /** Eigene Reihenfolge von „Meine Aufgaben“ (je Benutzer lokal) */
  get inboxOrderFile(): string { return join(this.dir, 'inbox-order.json'); }
  get dispatchDir(): string { return join(this.dir, 'dispatch'); }
  get dispatchRunsDir(): string { return join(this.dir, 'dispatch', 'runs'); }
  get dispatchStateFile(): string { return join(this.dir, 'dispatch', 'state.json'); }
  /** Review-Einträge vor dem Merge (<Ticket-ID>.json) */
  get dispatchReviewsDir(): string { return join(this.dir, 'dispatch', 'reviews'); }
  /** Leerer Arbeitsordner der Claude-Analyse (Optimierungs-Vorschläge) */
  get dispatchAnalysisDir(): string { return join(this.dir, 'dispatch', 'analysis'); }
  get dispatchLogsDir(): string { return join(this.dir, 'dispatch-logs'); }
  /** Zentrale Regeln, die die Projekte über ihre Rule importieren (z. B. `docs.md`) */
  get rulesDir(): string { return join(this.dir, 'rules'); }
  /** Angewendete Anpassungen (Workflow, Übersicht), geschrieben von `harness install` */
  get customizationsDir(): string { return join(this.dir, 'customizations'); }
}
