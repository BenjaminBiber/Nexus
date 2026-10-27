import { existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TempRepo } from '../helpers/TempRepo.js';
import { TargetBranches } from '../../../src/dispatch/git/TargetBranches.js';
import { GitRunner } from '../../../src/git/GitRunner.js';
import { PathKey } from '../../../src/git/PathKey.js';

describe('TargetBranches', () => {
  let repo: TempRepo;
  const t = new TargetBranches();
  const keys = new PathKey();

  // Ort eines Branches in vergleichbarer Schreibweise (Git liefert Schrägstriche, Tests Rückstriche)
  const placeOf = (name: string): string | null => {
    const place = t.checkedOutAt(repo.path, name);
    return place === null ? null : keys.of(place);
  };

  beforeEach(() => {
    repo = new TempRepo();
    repo.git('config', 'user.name', 't');
    repo.git('config', 'user.email', 't@t');
  });
  afterEach(() => repo.dispose());

  it('legt einen fehlenden Zielbranch vom HEAD des Hauptordners an', () => {
    const head = repo.git('rev-parse', 'HEAD');
    expect(t.ensure(repo.path, 'release/1')).toEqual({ created: true, from: repo.git('branch', '--show-current') });
    expect(repo.git('rev-parse', 'refs/heads/release/1')).toBe(head);
    expect(t.ensure(repo.path, 'release/1')).toEqual({ created: false, from: null });
  });

  it('nimmt bei losgelöstem HEAD den Kurz-Hash als Herkunft', () => {
    repo.git('checkout', '-q', '--detach');
    expect(t.ensure(repo.path, 'release/1')).toEqual({ created: true, from: repo.git('rev-parse', '--short', 'HEAD') });
  });

  it('nimmt origin/<name>, wenn es ihn gibt', () => {
    repo.git('commit', '--allow-empty', '-q', '-m', 'remote');
    const remote = repo.git('rev-parse', 'HEAD');
    repo.git('update-ref', 'refs/remotes/origin/release/2', remote);
    repo.git('reset', '-q', '--hard', 'HEAD~1');
    expect(t.ensure(repo.path, 'release/2')).toEqual({ created: true, from: 'origin/release/2' });
    expect(repo.git('rev-parse', 'refs/heads/release/2')).toBe(remote);
  });

  it('setzt beim Anlegen aus origin/<name> keinen Upstream', () => {
    repo.git('remote', 'add', 'origin', 'https://example.invalid/repo.git');
    repo.git('update-ref', 'refs/remotes/origin/release/2', repo.git('rev-parse', 'HEAD'));
    t.ensure(repo.path, 'release/2');
    expect(new GitRunner().tryRun(repo.path, ['config', '--get-regexp', '^branch\\.release/2\\.']).out).toBe('');
  });

  it('findet den Ort eines ausgecheckten Branches', () => {
    expect(placeOf(repo.git('branch', '--show-current'))).toBe(keys.of(repo.path));
    const wt = repo.addWorktree('w1');
    expect(placeOf('branch-w1')).toBe(keys.of(wt));
    t.ensure(repo.path, 'release/3');
    expect(t.checkedOutAt(repo.path, 'release/3')).toBeNull();
  });

  it('öffnet, verwendet wieder und schließt den Merge-Worktree', () => {
    t.ensure(repo.path, 'release/4');
    const path = t.openMergeWorktree(repo.path, 'release/4');
    expect(path).toBe(t.mergeWorktreePath(repo.path, 'release/4'));
    expect(path.endsWith('-worktrees/_merge/release-4')).toBe(true);
    expect(t.openMergeWorktree(repo.path, 'release/4')).toBe(path);
    expect(placeOf('release/4')).toBe(keys.of(path));
    expect(t.closeMergeWorktree(repo.path, path)).toBeNull();
    expect(existsSync(path)).toBe(false);
  });

  it('lehnt einen fremden Ordner am Ort des Merge-Worktrees ab', () => {
    t.ensure(repo.path, 'release/5');
    mkdirSync(t.mergeWorktreePath(repo.path, 'release/5'), { recursive: true });
    expect(() => t.openMergeWorktree(repo.path, 'release/5')).toThrow(expect.objectContaining({ code: 'merge-worktree-foreign' }));
  });

  it('lehnt einen Worktree eines anderen Branches am Ort des Merge-Worktrees ab', () => {
    t.ensure(repo.path, 'release/6');
    t.ensure(repo.path, 'release/7');
    const path = t.mergeWorktreePath(repo.path, 'release/6');
    repo.git('worktree', 'add', '-q', path, 'release/7');
    expect(() => t.openMergeWorktree(repo.path, 'release/6')).toThrow(expect.objectContaining({ code: 'merge-worktree-foreign' }));
  });

  it('legt den Merge-Worktree neu an, wenn sein Ordner von Hand gelöscht wurde', () => {
    t.ensure(repo.path, 'release/8');
    const path = t.openMergeWorktree(repo.path, 'release/8');
    rmSync(path, { recursive: true, force: true });
    expect(t.openMergeWorktree(repo.path, 'release/8')).toBe(path);
    expect(placeOf('release/8')).toBe(keys.of(path));
  });

  it('meldet, wenn der Merge-Worktree nicht entfernt werden kann', () => {
    t.ensure(repo.path, 'release/9');
    const path = t.openMergeWorktree(repo.path, 'release/9');
    // Eine unversionierte Datei hindert `worktree remove` ohne --force
    writeFileSync(`${path}/neu.txt`, 'x');
    expect(t.closeMergeWorktree(repo.path, path)).toMatch(/--force/);
    expect(existsSync(path)).toBe(true);
  });
});
