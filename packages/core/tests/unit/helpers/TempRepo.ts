import { mkdirSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { makeTempDir } from './TempDir.js';

// Temporäres Git-Projekt für Tests (Ordnername mit Leerzeichen und Umlaut), optional mit Worktrees.
export class TempRepo {
  readonly base: string;
  readonly path: string;

  // Legt das Projekt an und erzeugt auf Wunsch einen ersten Commit
  constructor(withCommit = true) {
    this.base = makeTempDir('harness-core-');
    this.path = join(this.base, 'Mein Projekt ä');
    mkdirSync(this.path);
    this.git('init', '-q');
    if (withCommit) this.git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '--allow-empty', '-q', '-m', 'init');
  }

  // Führt Git im Projekt aus
  git(...args: string[]): string {
    return execFileSync('git', args, { cwd: this.path, encoding: 'utf8', windowsHide: true }).trim();
  }

  // Legt einen zusätzlichen Worktree neben dem Projekt an und liefert seinen Pfad
  addWorktree(name: string): string {
    const path = join(this.base, name);
    this.git('worktree', 'add', '-q', '-b', `branch-${name}`, path);
    return path;
  }

  // Räumt das temporäre Verzeichnis auf
  dispose(): void {
    rmSync(this.base, { recursive: true, force: true });
  }
}
