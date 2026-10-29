import { statSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';

/** Woher der Ordner kommt; dieselben Kennungen wie in `NexusHome` (Rust). */
export type HomeSource = 'env-nexus' | 'env-harness' | 'nexus' | 'harness-fallback' | 'new';

// Benutzerweiter Ordner von Nexus (`~/.nexus`, überschreibbar mit NEXUS_HOME). In der
// Übergangszeit zählen `HARNESS_HOME` und `~/.harness` noch als Rückfall; die Reihenfolge ist
// dieselbe wie in `crates/nexus-core/src/registry/home.rs`, damit Node-Server und Rust-CLI
// denselben Ordner (und dieselbe board.db) nehmen.
export class HarnessHome {
  readonly dir: string;
  /** Woher der Ordner kommt (für `nexus migrate` und die Hinweise der CLI) */
  readonly source: HomeSource;

  constructor(env: Record<string, string | undefined> = process.env, base: string = homedir()) {
    const resolved = HarnessHome.resolve(env, base);
    this.dir = resolved.dir;
    this.source = resolved.source;
  }

  // Reihenfolge: NEXUS_HOME, HARNESS_HOME (veraltet), vorhandenes ~/.nexus, vorhandenes
  // ~/.harness (Rückfall), sonst ~/.nexus (Neuinstallation). Leere Werte zählen wie keine.
  private static resolve(
    env: Record<string, string | undefined>,
    base: string,
  ): { dir: string; source: HomeSource } {
    if (env.NEXUS_HOME) return { dir: env.NEXUS_HOME, source: 'env-nexus' };
    if (env.HARNESS_HOME) return { dir: env.HARNESS_HOME, source: 'env-harness' };
    const nexus = join(base, '.nexus');
    if (HarnessHome.isDir(nexus)) return { dir: nexus, source: 'nexus' };
    const harness = join(base, '.harness');
    if (HarnessHome.isDir(harness)) return { dir: harness, source: 'harness-fallback' };
    return { dir: nexus, source: 'new' };
  }

  // Gibt es den Ordner?
  private static isDir(dir: string): boolean {
    try {
      return statSync(dir).isDirectory();
    } catch {
      return false;
    }
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
  /** Angewendete Anpassungen (Workflow, Übersicht), geschrieben von `nexus install` */
  get customizationsDir(): string { return join(this.dir, 'customizations'); }
}
