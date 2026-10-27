import { existsSync } from 'node:fs';
import { resolve, sep } from 'node:path';
import type { GitRunner } from '../git/GitRunner.js';
import { PROJECT_RULE_PATHS } from '../install/ProjectRules.js';
import { WorktreeLinks } from './WorktreeLinks.js';
import type { WorktreeEntry, WorktreeRegistry } from './WorktreeRegistry.js';

// Lokale Dateien, die der Harness selbst im Worktree anlegt (Plugin-Einstellung, Rules für Doku- und Board-Regeln);
// sie zählen nicht als Änderung, auch wenn ein älteres Projekt sie noch nicht in `info/exclude` führt.
const HARNESS_FILES = new Set(['.claude/settings.local.json', ...PROJECT_RULE_PATHS]);

// Räumt Worktrees auf, deren Tickets alle erledigt sind. Der Branch bleibt, es wird nie gemergt (ADR 0002).
export class WorktreeCleaner {
  private readonly links = new WorktreeLinks();

  constructor(
    private readonly registry: WorktreeRegistry,
    private readonly git: GitRunner,
  ) {}

  // Entfernt alle bereiten, sauberen Worktrees, in denen `cwd` nicht liegt; liefert die entfernten Pfade. Vorher werden
  // Symlinks/Junctions im Worktree entfernt (nur die Links, nie ihr Ziel); bleibt ein Link stehen oder scheitert Git,
  // bleibt der Worktree stehen (kein Abbruch von start/done)
  async cleanup(cwd: string): Promise<string[]> {
    const removed: string[] = [];
    for (const [branch, entry] of Object.entries(await this.registry.read())) {
      if (!this.removable(entry, cwd)) continue;
      if (this.links.removeAll(entry.path).failed.length) continue;
      // --force nur, weil die lokalen Dateien des Harness nicht im Git liegen; Links gibt es jetzt keine mehr
      if (!this.git.tryRun(cwd, ['worktree', 'remove', '--force', entry.path]).ok) continue;
      await this.registry.markRemoved(branch);
      removed.push(entry.path);
    }
    return removed;
  }

  // Bereit zum Merge, noch vorhanden, nicht der aktuelle Ordner und ohne eigene Änderungen
  private removable(entry: WorktreeEntry, cwd: string): boolean {
    if (!entry.readyToMerge || entry.removed || !existsSync(entry.path)) return false;
    if (this.contains(entry.path, cwd)) return false;
    return this.isClean(entry.path);
  }

  // Ist `path` der Ordner `outer` selbst oder liegt darin (Groß-/Kleinschreibung egal, wie unter Windows)
  private contains(outer: string, path: string): boolean {
    const a = resolve(outer).toLowerCase();
    const b = resolve(path).toLowerCase();
    return b === a || b.startsWith(a + sep);
  }

  // Sauber: keine geänderten oder neuen Dateien außer den lokalen Dateien des Harness
  private isClean(path: string): boolean {
    try {
      const lines = this.git.run(path, ['status', '--porcelain', '--untracked-files=all']).split('\n');
      // Die Ausgabe ist getrimmt: Statuskürzel ("M", "??") abschneiden statt feste Spalten anzunehmen
      const files = lines.filter((l) => l.trim() !== '').map((l) => l.trim().replace(/^\S+\s+/, ''));
      return files.every((f) => HARNESS_FILES.has(f.replace(/\\/g, '/')));
    } catch {
      return false;
    }
  }
}
