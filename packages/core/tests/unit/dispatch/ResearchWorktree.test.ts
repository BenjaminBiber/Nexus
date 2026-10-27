import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { ResearchWorktree } from '../../../src/dispatch/git/ResearchWorktree.js';
import { TempRepo } from '../helpers/TempRepo.js';

describe('ResearchWorktree', () => {
  let repo: TempRepo;
  afterEach(() => repo?.dispose());

  it('legt einen losgelösten Worktree neben dem Repo an und entfernt nur ihn wieder', async () => {
    repo = new TempRepo();
    const worktrees = new ResearchWorktree();
    const { path } = await worktrees.create(repo.path, 'ARC-F0007');
    expect(path).toBe(join(repo.base, 'Mein Projekt ä-worktrees', 'research-ARC-F0007').replace(/\\/g, '/'));
    expect(existsSync(path)).toBe(true);
    expect(repo.git('-C', path, 'rev-parse', '--abbrev-ref', 'HEAD')).toBe('HEAD');
    expect(repo.git('branch', '--list')).not.toContain('research');
    writeFileSync(join(path, 'research.json'), '{}');
    expect(worktrees.remove(repo.path, path)).toBeNull();
    expect(existsSync(path)).toBe(false);
    expect(repo.git('worktree', 'list')).not.toContain('research-ARC-F0007');
  });

  it('ersetzt einen Rest eines früheren Laufs und fasst fremde Ordner nicht an', async () => {
    repo = new TempRepo();
    const worktrees = new ResearchWorktree();
    const first = await worktrees.create(repo.path, 'ARC-F0001');
    const second = await worktrees.create(repo.path, 'ARC-F0001');
    expect(second.path).toBe(first.path);
    expect(existsSync(second.path)).toBe(true);
    const other = join(repo.base, 'anderes');
    mkdirSync(other);
    expect(worktrees.remove(repo.path, other)).toMatch(/kein Recherche-Worktree/);
    expect(worktrees.remove(repo.path, repo.path)).toMatch(/kein Recherche-Worktree/);
    expect(existsSync(other)).toBe(true);
    // Ein Ordner research-<ID> an anderem Platz (nicht unter <repo>-worktrees) wird nie angefasst
    const elsewhere = join(repo.base, 'research-ARC-F0002');
    mkdirSync(elsewhere);
    expect(worktrees.remove(repo.path, elsewhere)).toMatch(/kein Recherche-Worktree/);
    expect(existsSync(elsewhere)).toBe(true);
    worktrees.remove(repo.path, second.path);
  });

  it('entfernt verwaiste Recherche-Worktrees ohne Lauf, laufende und fremde Worktrees bleiben', async () => {
    repo = new TempRepo();
    const worktrees = new ResearchWorktree();
    const orphan = await worktrees.create(repo.path, 'ARC-F0001');
    const running = await worktrees.create(repo.path, 'ARC-F0002');
    const foreign = join(repo.base, 'research-ARC-F0003').replace(/\\/g, '/');
    repo.git('worktree', 'add', '--detach', foreign, 'HEAD');
    const notes = worktrees.removeOrphans(repo.path, [running.path]);
    expect(notes).toEqual([expect.stringMatching(/^verwaister Recherche-Worktree .*-worktrees\/research-ARC-F0001 entfernt$/)]);
    expect(existsSync(orphan.path)).toBe(false);
    expect(existsSync(running.path)).toBe(true);
    expect(existsSync(foreign)).toBe(true);
    expect(worktrees.removeOrphans(repo.path, [running.path])).toEqual([]);
    expect(worktrees.removeOrphans(join(repo.base, 'kein-repo'), [])).toEqual([]);
    worktrees.remove(repo.path, running.path);
    repo.git('worktree', 'remove', '--force', foreign);
  });

  it('erkennt laufende Worktrees auch bei abweichender Pfadschreibweise', async () => {
    repo = new TempRepo();
    const worktrees = new ResearchWorktree();
    const running = await worktrees.create(repo.path, 'ARC-F0004');
    // Unter Windows andere Groß-/Kleinschreibung und Rückstriche, sonst ein Umweg über „/./“
    const spelled = process.platform === 'win32'
      ? running.path.toUpperCase().replace(/\//g, '\\')
      : running.path.replace(/\/research-/, '/./research-');
    expect(worktrees.removeOrphans(repo.path, [spelled])).toEqual([]);
    expect(existsSync(running.path)).toBe(true);
    worktrees.remove(repo.path, running.path);
  });

  it('Präfix plan: eigener Ordner plan-<ID>, Aufräumen trennt Planungs- und Recherche-Worktrees', async () => {
    repo = new TempRepo();
    const plans = new ResearchWorktree('plan');
    const research = new ResearchWorktree();
    const plan = await plans.create(repo.path, 'ARC-0007');
    expect(plan.path).toBe(join(repo.base, 'Mein Projekt ä-worktrees', 'plan-ARC-0007').replace(/\\/g, '/'));
    const question = await research.create(repo.path, 'ARC-F0007');
    expect(research.remove(repo.path, plan.path)).toMatch(/kein Recherche-Worktree/);
    expect(plans.remove(repo.path, question.path)).toMatch(/kein Planungs-Worktree/);
    expect(research.removeOrphans(repo.path, [question.path])).toEqual([]);
    expect(plans.removeOrphans(repo.path, [])).toEqual([expect.stringMatching(/^verwaister Planungs-Worktree .*-worktrees\/plan-ARC-0007 entfernt$/)]);
    expect(existsSync(plan.path)).toBe(false);
    expect(existsSync(question.path)).toBe(true);
    research.remove(repo.path, question.path);
  });
});
