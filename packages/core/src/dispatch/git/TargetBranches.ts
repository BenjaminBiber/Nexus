import { existsSync } from 'node:fs';
import { basename, dirname, join } from 'node:path';
import { HarnessError } from '../../errors/HarnessError.js';
import { GitRunner } from '../../git/GitRunner.js';
import { LONG_PATHS_ARGS } from '../../git/LongPaths.js';
import { PathKey } from '../../git/PathKey.js';

/** Ergebnis von `ensure`: `from` nennt die Herkunft eines neu angelegten Branches (sonst null) */
export interface EnsuredBranch {
  created: boolean;
  from: string | null;
}

// Git-Helfer für Zielbranches: lokal anlegen, Ort des Checkouts finden und der Merge-Worktree, in dem ein Ticket in
// seinen Zielbranch gemergt wird, ohne den Hauptordner des Nutzers anzufassen. Kein Push, kein Fetch.
export class TargetBranches {
  private readonly git = new GitRunner();
  private readonly keys = new PathKey();

  // Legt den Branch lokal an, falls er fehlt: von origin/<name>, sonst vom HEAD des Hauptordners (immer ohne Upstream)
  ensure(root: string, name: string): EnsuredBranch {
    if (this.exists(root, `refs/heads/${name}`)) return { created: false, from: null };
    if (this.exists(root, `refs/remotes/origin/${name}`)) {
      this.git.run(root, ['branch', '--no-track', name, `refs/remotes/origin/${name}`]);
      return { created: true, from: `origin/${name}` };
    }
    const current = this.git.run(root, ['branch', '--show-current']);
    const from = current || this.git.run(root, ['rev-parse', '--short', 'HEAD']);
    this.git.run(root, ['branch', '--no-track', name, 'HEAD']);
    return { created: true, from };
  }

  // Ordner, in dem der Branch ausgecheckt ist (Hauptordner oder Worktree), so wie Git ihn nennt – Vergleiche über PathKey –, sonst null
  checkedOutAt(root: string, name: string): string | null {
    let current: string | null = null;
    for (const line of this.git.run(root, ['worktree', 'list', '--porcelain']).split(/\r?\n/)) {
      if (line.startsWith('worktree ')) current = line.slice('worktree '.length);
      else if (line === `branch refs/heads/${name}` && current !== null) return current;
      else if (line === '') current = null;
    }
    return null;
  }

  // Ordner des Merge-Worktrees eines Zielbranches: <repo>-worktrees/_merge/<Branch, „/“ als „-“>
  mergeWorktreePath(root: string, name: string): string {
    return join(dirname(root), `${basename(root)}-worktrees`, '_merge', name.replace(/\//g, '-')).replace(/\\/g, '/');
  }

  // Öffnet den Merge-Worktree des Zielbranches: ein vorhandener Worktree dieses Branches wird wiederverwendet, ein anderer
  // Ordner am selben Platz abgelehnt, sonst (nach `worktree prune`) neu angelegt
  openMergeWorktree(root: string, name: string): string {
    const path = this.mergeWorktreePath(root, name);
    if (existsSync(path)) {
      if (this.isWorktreeOf(root, path, name)) return path;
      throw new HarnessError(
        'merge-worktree-foreign',
        `Der Ordner ${path} existiert schon und ist kein Worktree von Branch ${name}. Bitte den Ordner prüfen und von Hand aufräumen.`,
      );
    }
    this.git.run(root, ['worktree', 'prune']);
    this.git.run(root, [...LONG_PATHS_ARGS, 'worktree', 'add', path, name]);
    return path;
  }

  // Entfernt den Merge-Worktree ohne Gewalt (ungesicherte Änderungen bleiben stehen); liefert die Git-Meldung bei Fehlschlag, sonst null
  closeMergeWorktree(root: string, path: string): string | null {
    const attempt = this.git.tryRun(root, ['worktree', 'remove', path]);
    return attempt.ok ? null : attempt.err;
  }

  // Gibt es die Referenz?
  private exists(root: string, ref: string): boolean {
    return this.git.tryRun(root, ['rev-parse', '--verify', '--quiet', ref]).ok;
  }

  // Ist der Ordner in `git worktree list` als Worktree des Branches eingetragen?
  private isWorktreeOf(root: string, path: string, name: string): boolean {
    const place = this.checkedOutAt(root, name);
    return place !== null && this.keys.same(place, path);
  }
}
