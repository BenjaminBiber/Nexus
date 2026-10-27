import { GitRunner } from './GitRunner.js';

// Hinweis, wenn die Einstellung neu gesetzt wurde
export const LONG_PATHS_NOTICE = 'core.longpaths im Repo eingeschaltet, damit lange Pfade im Worktree funktionieren.';

// Git-Argumente, die beim `worktree add` selbst lange Pfade erlauben (falls das Setzen der Konfiguration scheitert)
export const LONG_PATHS_ARGS: readonly string[] = ['-c', 'core.longpaths=true'];

// Schaltet unter Windows `core.longpaths` in der lokalen Repo-Konfiguration ein: Harness-Worktrees liegen unter
// <repo>-worktrees/<ID> und sind damit oft tiefer als der Hauptordner, sonst scheitert `git worktree add` an langen Pfaden.
// Die Einstellung gilt für das Repo und alle seine Worktrees.
export class LongPaths {
  private readonly git = new GitRunner();

  // Setzt die Einstellung, falls sie unter Windows fehlt; liefert true, wenn sie neu gesetzt wurde (sonst false, auch bei Fehlern)
  ensure(repoRoot: string, platform: string = process.platform): boolean {
    if (platform !== 'win32') return false;
    const current = this.git.tryRun(repoRoot, ['config', '--get', 'core.longpaths']);
    if (current.ok && current.out.toLowerCase() === 'true') return false;
    return this.git.tryRun(repoRoot, ['config', 'core.longpaths', 'true']).ok;
  }
}
