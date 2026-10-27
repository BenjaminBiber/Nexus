import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdirSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { GitPaths } from '../../../src/git/GitPaths.js';
import { HarnessError } from '../../../src/errors/HarnessError.js';
import { makeTempDir } from '../helpers/TempDir.js';

// Führt Git im angegebenen Ordner aus (Testhilfe)
function git(cwd: string, ...args: string[]): void {
  execFileSync('git', args, { cwd, stdio: 'ignore', windowsHide: true });
}

// Vereinheitlicht Pfade auf Schrägstriche, wie GitPaths es tut
function slashes(path: string): string {
  return path.split('\\').join('/');
}

describe('GitPaths', () => {
  let base: string;
  let repo: string;

  beforeEach(() => {
    base = makeTempDir('harness-git-');
    // Ordnername mit Leerzeichen und Umlaut, wie unter Windows üblich
    repo = join(base, 'Mein Projekt ä');
    mkdirSync(repo);
    git(repo, 'init', '-q');
    git(repo, '-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '--allow-empty', '-q', '-m', 'init');
  });

  afterEach(() => rmSync(base, { recursive: true, force: true }));

  it('liefert das gemeinsame Git-Verzeichnis im Hauptordner', () => {
    expect(new GitPaths().commonDir(repo)).toBe(slashes(join(repo, '.git')));
  });

  it('liefert aus einem Worktree dasselbe gemeinsame Verzeichnis', () => {
    const worktree = join(base, 'wt');
    git(repo, 'worktree', 'add', '-q', '-b', 'x', worktree);
    expect(new GitPaths().commonDir(worktree)).toBe(new GitPaths().commonDir(repo));
  });

  it('liefert die Wurzel des Hauptordners auch aus einem Worktree', () => {
    const worktree = join(base, 'wt');
    git(repo, 'worktree', 'add', '-q', '-b', 'y', worktree);
    expect(new GitPaths().mainRoot(worktree)).toBe(slashes(repo));
  });

  it('wirft einen verständlichen Fehler außerhalb eines Git-Projekts', () => {
    const plain = join(base, 'kein-git');
    mkdirSync(plain);
    expect(() => new GitPaths().commonDir(plain)).toThrow(HarnessError);
    expect(() => new GitPaths().commonDir(plain)).toThrow(/kein Git-Projekt/);
  });

  it('liefert asynchron dasselbe gemeinsame Verzeichnis wie synchron', async () => {
    const worktree = join(base, 'wt');
    git(repo, 'worktree', 'add', '-q', '-b', 'z', worktree);
    await expect(new GitPaths().commonDirAsync(worktree)).resolves.toBe(new GitPaths().commonDir(repo));
  });

  it('wirft asynchron denselben Fehler außerhalb eines Git-Projekts', async () => {
    const plain = join(base, 'kein-git');
    mkdirSync(plain);
    await expect(new GitPaths().commonDirAsync(plain)).rejects.toThrow(/kein Git-Projekt/);
  });
});
