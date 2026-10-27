import { chmodSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TempRepo } from '../helpers/TempRepo.js';
import { BranchMerger } from '../../../src/dispatch/git/BranchMerger.js';
import { GitRunner, type GitAttempt } from '../../../src/git/GitRunner.js';
import { HarnessError } from '../../../src/errors/HarnessError.js';

describe('BranchMerger', () => {
  let repo: TempRepo;
  let main: string;
  const merger = new BranchMerger();

  // Schreibt eine Datei und committet sie auf dem aktuellen Branch
  const commit = (file: string, content: string) => {
    writeFileSync(join(repo.path, file), content);
    repo.git('add', file);
    repo.git('commit', '-q', '-m', `${file}=${content}`);
  };

  beforeEach(() => {
    repo = new TempRepo();
    repo.git('config', 'user.name', 't');
    repo.git('config', 'user.email', 't@t');
    commit('a.txt', 'basis');
    main = repo.git('rev-parse', '--abbrev-ref', 'HEAD');
  });
  afterEach(() => repo.dispose());

  it('mergt ohne Konflikt mit eigenem Merge-Commit und löscht den Branch', () => {
    repo.git('checkout', '-q', '-b', 'ticket/x');
    commit('b.txt', 'neu');
    repo.git('checkout', '-q', main);
    const result = merger.merge(repo.path, 'ticket/x');
    expect(result.kind).toBe('merged');
    expect(repo.git('log', '-1', '--format=%P').split(' ')).toHaveLength(2);
    expect(merger.contains(repo.path, 'ticket/x', main)).toBe(true);
    expect(merger.deleteBranch(repo.path, 'ticket/x')).toBeNull();
    expect(repo.git('branch', '--list', 'ticket/x')).toBe('');
  });

  it('nutzt eine übergebene Merge-Nachricht statt der Git-Standardnachricht', () => {
    repo.git('checkout', '-q', '-b', 'ticket/x');
    commit('b.txt', 'neu');
    repo.git('checkout', '-q', main);
    expect(merger.merge(repo.path, 'ticket/x', 'Merge ARC-0012 „Login“ (ticket/x)').kind).toBe('merged');
    expect(repo.git('log', '-1', '--format=%s')).toBe('Merge ARC-0012 „Login“ (ticket/x)');
  });

  it('mergedEarlier: findet den Merge- bzw. Squash-Commit eines gelöschten Branches, aber keinen Revert oder bloße Erwähnung', () => {
    repo.git('checkout', '-q', '-b', 'ticket/x');
    commit('b.txt', 'neu');
    repo.git('checkout', '-q', main);
    commit('c.txt', 'erwähnt (ticket/x)');
    expect(merger.merge(repo.path, 'ticket/x', 'Merge ARC-0012 „Login“ (ticket/x)').kind).toBe('merged');
    const merge = repo.git('rev-parse', '--short', 'HEAD');
    expect(merger.mergedEarlier(repo.path, 'ticket/x')).toBeNull();
    repo.git('branch', '-D', 'ticket/x');
    expect(merger.mergedEarlier(repo.path, 'ticket/x')).toBe(merge);
    repo.git('revert', '--no-edit', '-m', '1', 'HEAD');
    expect(repo.git('log', '-1', '--format=%s')).toContain('(ticket/x)');
    expect(merger.mergedEarlier(repo.path, 'ticket/x')).toBe(merge);

    // Nur Erwähnung und Revert: kein früherer Merge
    repo.git('checkout', '-q', '-b', 'ticket/y');
    commit('d.txt', 'neu');
    repo.git('checkout', '-q', main);
    commit('e.txt', 'Revert "Merge ARC-0013 (ticket/y)"');
    repo.git('branch', '-D', 'ticket/y');
    expect(merger.mergedEarlier(repo.path, 'ticket/y')).toBeNull();

    // Squash: der Trailer zählt
    repo.git('checkout', '-q', '-b', 'ticket/z');
    commit('f.txt', 'neu');
    repo.git('checkout', '-q', main);
    expect(merger.merge(repo.path, 'ticket/z', 'ARC-0014 (squash von ticket/z)', 'squash').kind).toBe('merged');
    const squash = repo.git('rev-parse', '--short', 'HEAD');
    repo.git('branch', '-D', 'ticket/z');
    expect(merger.mergedEarlier(repo.path, 'ticket/z')).toBe(squash);
  });

  it('meldet Konflikte mit Dateien und bricht den Merge ab', () => {
    repo.git('checkout', '-q', '-b', 'ticket/x');
    commit('a.txt', 'ticket');
    repo.git('checkout', '-q', main);
    commit('a.txt', 'haupt');
    const head = merger.head(repo.path, 'HEAD');
    expect(merger.merge(repo.path, 'ticket/x')).toEqual({ kind: 'conflict', files: ['a.txt'] });
    expect(merger.head(repo.path, 'HEAD')).toBe(head);
    expect(repo.git('status', '--porcelain')).toBe('');
    expect(merger.contains(repo.path, main, 'ticket/x')).toBe(false);
  });

  it('meldet einen abgewiesenen Merge, wenn eine lokal geänderte Datei betroffen ist', () => {
    commit('cfg.yaml', 'a');
    repo.git('checkout', '-q', '-b', 'ticket/x');
    commit('cfg.yaml', 'b');
    repo.git('checkout', '-q', main);
    writeFileSync(join(repo.path, 'cfg.yaml'), 'lokal');
    const result = merger.merge(repo.path, 'ticket/x');
    expect(result.kind).toBe('refused');
    expect((result as { message: string }).message).toContain('cfg.yaml');
    expect(repo.git('status', '--porcelain')).toBe('M cfg.yaml');
  });

  it('overlapping: Schnittmenge aus Branch-Änderungen und lokalen (auch neuen) Dateien, ohne die des Hauptbranches', () => {
    commit('cfg.yaml', 'a');
    commit('frei.txt', 'a');
    repo.git('checkout', '-q', '-b', 'ticket/x');
    commit('cfg.yaml', 'b');
    commit('neu.txt', 'ticket');
    commit('nur-ticket.txt', 'ticket');
    repo.git('checkout', '-q', main);
    commit('haupt.txt', 'haupt');
    writeFileSync(join(repo.path, 'cfg.yaml'), 'lokal');
    writeFileSync(join(repo.path, 'neu.txt'), 'lokal');
    writeFileSync(join(repo.path, 'frei.txt'), 'lokal');
    writeFileSync(join(repo.path, 'haupt.txt'), 'lokal');
    expect(merger.overlapping(repo.path, 'ticket/x')).toEqual(['cfg.yaml', 'neu.txt']);
  });

  it('overlapping: ohne lokale Änderungen leer', () => {
    repo.git('checkout', '-q', '-b', 'ticket/x');
    commit('b.txt', 'neu');
    repo.git('checkout', '-q', main);
    expect(merger.overlapping(repo.path, 'ticket/x')).toEqual([]);
  });

  it('meldet einen Fehler beim Löschen eines nicht gemergten Branches', () => {
    repo.git('checkout', '-q', '-b', 'ticket/y');
    commit('c.txt', 'x');
    repo.git('checkout', '-q', main);
    expect(merger.deleteBranch(repo.path, 'ticket/y')).toMatch(/ticket\/y/);
  });

  // Ticket-Branch und Hauptbranch ändern a.txt unterschiedlich
  const prepareConflict = () => {
    repo.git('checkout', '-q', '-b', 'ticket/x');
    commit('a.txt', 'ticket');
    repo.git('checkout', '-q', main);
    commit('a.txt', 'haupt');
  };
  const mergeHead = () => existsSync(join(repo.path, '.git', 'MERGE_HEAD'));

  it('weist ab, wenn im Hauptordner schon ein Merge läuft, und lässt ihn unangetastet', () => {
    prepareConflict();
    repo.git('checkout', '-q', '-b', 'ticket/y');
    commit('c.txt', 'y');
    repo.git('checkout', '-q', main);
    expect(() => repo.git('merge', '--no-edit', 'ticket/x')).toThrow();
    expect(mergeHead()).toBe(true);
    const result = merger.merge(repo.path, 'ticket/y');
    expect(result.kind).toBe('refused');
    expect((result as { message: string }).message).toContain('Im Hauptordner läuft bereits ein Merge');
    expect(mergeHead()).toBe(true);
    expect(existsSync(join(repo.path, 'c.txt'))).toBe(false);
  });

  it('weist bei laufendem Cherry-Pick oder Rebase ab, ohne zu mergen', () => {
    repo.git('checkout', '-q', '-b', 'ticket/x');
    commit('b.txt', 'neu');
    repo.git('checkout', '-q', main);
    const head = merger.head(repo.path, 'HEAD');
    writeFileSync(join(repo.path, '.git', 'CHERRY_PICK_HEAD'), head);
    expect(merger.merge(repo.path, 'ticket/x')).toMatchObject({ kind: 'refused', message: expect.stringContaining('Cherry-Pick') });
    rmSync(join(repo.path, '.git', 'CHERRY_PICK_HEAD'));
    mkdirSync(join(repo.path, '.git', 'rebase-merge'));
    expect(merger.merge(repo.path, 'ticket/x')).toMatchObject({ kind: 'refused', message: expect.stringContaining('Rebase') });
    expect(merger.head(repo.path, 'HEAD')).toBe(head);
  });

  it('bricht den Merge auch ab, wenn die Konfliktliste nicht gelesen werden kann', () => {
    prepareConflict();
    const git = new FakeGit((args) => (args.includes('diff') ? { ok: false, out: '', err: 'kaputt' } : null));
    const result = new BranchMerger(git).merge(repo.path, 'ticket/x');
    expect(result).toEqual({ kind: 'conflict', files: [] });
    expect(git.calls).toContainEqual(['merge', '--abort']);
    expect(mergeHead()).toBe(false);
    expect(repo.git('status', '--porcelain')).toBe('');
  });

  it('wirft merge-abort-failed, wenn der Abbruch scheitert', () => {
    prepareConflict();
    const git = new FakeGit((args) => (args.join(' ') === 'merge --abort' ? { ok: false, out: '', err: 'gesperrt' } : null));
    let error: unknown;
    try {
      new BranchMerger(git).merge(repo.path, 'ticket/x');
    } catch (e) {
      error = e;
    }
    expect(error).toBeInstanceOf(HarnessError);
    expect(error).toMatchObject({ code: 'merge-abort-failed' });
    expect((error as Error).message).toContain('gesperrt');
    expect((error as Error).message).toContain('git merge --abort');
    // Der Aufrufer kennt den Ort des Merges und nennt damit den richtigen Ordner
    expect((error as HarnessError).details).toEqual({ root: repo.path, git: 'gesperrt', command: 'git merge --abort' });
    expect(mergeHead()).toBe(true);
    repo.git('merge', '--abort');
  });

  // Ticket-Branch und Hauptbranch haken verschiedene Aufgaben derselben tasks.md ab – seit dem Wegfall des
  // Spec-Kit-Formats ist das ein gewöhnlicher Konflikt
  const prepareTasksConflict = () => {
    mkdirSync(join(repo.path, 'specs', '001-x'), { recursive: true });
    commit('specs/001-x/tasks.md', '# Aufgaben\n- [ ] T001 Erste\n- [ ] T002 Zweite\n');
    repo.git('checkout', '-q', '-b', 'ticket/x');
    commit('specs/001-x/tasks.md', '# Aufgaben\n- [X] T001 Erste\n- [ ] T002 Zweite\n');
    repo.git('checkout', '-q', main);
    commit('specs/001-x/tasks.md', '# Aufgaben\n- [ ] T001 Erste\n- [X] T002 Zweite\n');
  };

  it('meldet einen Konflikt in einer tasks.md wie bei jeder anderen Datei', () => {
    prepareTasksConflict();
    const head = merger.head(repo.path, 'HEAD');
    expect(merger.merge(repo.path, 'ticket/x')).toEqual({ kind: 'conflict', files: ['specs/001-x/tasks.md'] });
    expect(merger.head(repo.path, 'HEAD')).toBe(head);
    expect(mergeHead()).toBe(false);
    expect(repo.git('status', '--porcelain')).toBe('');
  });

  it('meldet einen Konflikt in zwei Dateien mit beiden Pfaden', () => {
    prepareConflict();
    repo.git('checkout', '-q', 'ticket/x');
    commit('b.txt', 'ticket');
    repo.git('checkout', '-q', main);
    commit('b.txt', 'haupt');
    const head = merger.head(repo.path, 'HEAD');
    expect(merger.merge(repo.path, 'ticket/x')).toEqual({ kind: 'conflict', files: ['a.txt', 'b.txt'] });
    expect(merger.head(repo.path, 'HEAD')).toBe(head);
    expect(mergeHead()).toBe(false);
    expect(repo.git('status', '--porcelain')).toBe('');
  });

  it('mergt den Branch, auch wenn ein gleichnamiger Tag existiert', () => {
    repo.git('tag', 'ticket/x');
    repo.git('checkout', '-q', '-b', 'ticket/x');
    commit('b.txt', 'neu');
    repo.git('checkout', '-q', main);
    expect(merger.merge(repo.path, 'ticket/x').kind).toBe('merged');
    expect(existsSync(join(repo.path, 'b.txt'))).toBe(true);
  });

  describe('Squash', () => {
    const leftovers = () => existsSync(join(repo.path, '.git', 'SQUASH_MSG')) || mergeHead();

    it('fasst alle Commits des Branches in einem Commit mit Liste und Trailer zusammen', () => {
      repo.git('checkout', '-q', '-b', 'ticket/x');
      commit('b.txt', 'eins');
      commit('c.txt', 'zwei');
      const tip = merger.head(repo.path, 'ticket/x');
      repo.git('checkout', '-q', main);
      const before = merger.head(repo.path, 'HEAD');
      const result = merger.merge(repo.path, 'ticket/x', 'ARC-0012 „Login“ (squash von ticket/x)', 'squash');
      expect(result.kind).toBe('merged');
      expect(repo.git('log', '-1', '--format=%P')).toBe(before);
      expect(repo.git('log', '-1', '--format=%s')).toBe('ARC-0012 „Login“ (squash von ticket/x)');
      const body = repo.git('log', '-1', '--format=%b');
      expect(body).toContain('- b.txt=eins');
      expect(body).toContain('- c.txt=zwei');
      expect(repo.git('log', '-1', '--format=%(trailers:key=Harness-Squashed,valueonly)')).toBe(`ticket/x ${tip}`);
      expect(repo.git('show', 'HEAD:c.txt')).toBe('zwei');
      expect(repo.git('status', '--porcelain')).toBe('');
      expect(leftovers()).toBe(false);
      expect(merger.contains(repo.path, 'ticket/x', main)).toBe(false);
      expect(merger.integrated(repo.path, 'ticket/x')).toBe(true);
    });

    it('kürzt die Liste der zusammengefassten Commits', () => {
      repo.git('checkout', '-q', '-b', 'ticket/x');
      for (let i = 0; i < 60; i += 1) commit('b.txt', `v${i}`);
      repo.git('checkout', '-q', main);
      expect(merger.merge(repo.path, 'ticket/x', 'ARC-0012 (squash von ticket/x)', 'squash').kind).toBe('merged');
      const body = repo.git('log', '-1', '--format=%b');
      expect(body.split('\n').filter((l) => l.startsWith('- '))).toHaveLength(50);
      expect(body).toContain('und 10 weitere');
    });

    it('meldet einen Konflikt in einer tasks.md wie bei jeder anderen Datei', () => {
      prepareTasksConflict();
      const before = merger.head(repo.path, 'HEAD');
      expect(merger.merge(repo.path, 'ticket/x', 'ARC-0012 (squash von ticket/x)', 'squash')).toEqual({
        kind: 'conflict',
        files: ['specs/001-x/tasks.md'],
      });
      expect(merger.head(repo.path, 'HEAD')).toBe(before);
      expect(repo.git('status', '--porcelain')).toBe('');
      expect(leftovers()).toBe(false);
    });

    it('meldet einen echten Konflikt und lässt den Hauptordner samt lokaler Änderungen exakt stehen', () => {
      commit('cfg.yaml', 'a');
      prepareConflict();
      repo.git('checkout', '-q', 'ticket/x');
      commit('b.txt', 'neu');
      repo.git('checkout', '-q', main);
      writeFileSync(join(repo.path, 'cfg.yaml'), 'lokal');
      writeFileSync(join(repo.path, 'frei.txt'), 'neu');
      const head = merger.head(repo.path, 'HEAD');
      const status = repo.git('status', '--porcelain');
      expect(merger.merge(repo.path, 'ticket/x', 'm', 'squash')).toEqual({ kind: 'conflict', files: ['a.txt'] });
      expect(merger.head(repo.path, 'HEAD')).toBe(head);
      expect(repo.git('status', '--porcelain')).toBe(status);
      expect(repo.git('show', ':a.txt')).toBe('haupt');
      expect(existsSync(join(repo.path, 'b.txt'))).toBe(false);
      expect(repo.git('show', ':cfg.yaml')).toBe('a');
      expect(leftovers()).toBe(false);
      expect(merger.integrated(repo.path, 'ticket/x')).toBe(false);
    });

    it('weist ab, wenn eine lokal geänderte Datei betroffen ist, ohne etwas zu verändern', () => {
      commit('cfg.yaml', 'a');
      repo.git('checkout', '-q', '-b', 'ticket/x');
      commit('cfg.yaml', 'b');
      repo.git('checkout', '-q', main);
      writeFileSync(join(repo.path, 'cfg.yaml'), 'lokal');
      const head = merger.head(repo.path, 'HEAD');
      const result = merger.merge(repo.path, 'ticket/x', 'm', 'squash');
      expect(result.kind).toBe('refused');
      expect((result as { message: string }).message).toContain('cfg.yaml');
      expect(repo.git('status', '--porcelain')).toBe('M cfg.yaml');
      expect(merger.head(repo.path, 'HEAD')).toBe(head);
    });

    it('weist bei laufendem Merge ab, ohne ihn anzufassen', () => {
      prepareConflict();
      repo.git('checkout', '-q', '-b', 'ticket/y');
      commit('c.txt', 'y');
      repo.git('checkout', '-q', main);
      expect(() => repo.git('merge', '--no-edit', 'ticket/x')).toThrow();
      expect(merger.merge(repo.path, 'ticket/y', 'm', 'squash').kind).toBe('refused');
      expect(mergeHead()).toBe(true);
      repo.git('merge', '--abort');
    });

    it('legt für einen Branch ohne neue Änderungen einen leeren Commit mit Trailer an', () => {
      repo.git('checkout', '-q', '-b', 'ticket/x');
      commit('b.txt', 'neu');
      const tip = merger.head(repo.path, 'ticket/x');
      repo.git('checkout', '-q', main);
      // Eigener Commit vorher: sonst ergäbe der Cherry-Pick in derselben Sekunde denselben Commit wie im Branch
      commit('haupt.txt', 'haupt');
      repo.git('cherry-pick', 'ticket/x');
      const before = merger.head(repo.path, 'HEAD');
      const result = merger.merge(repo.path, 'ticket/x', 'ARC-0012 (squash von ticket/x)', 'squash');
      expect(result.kind).toBe('merged');
      expect(repo.git('log', '-1', '--format=%P')).toBe(before);
      expect(repo.git('log', '-1', '--format=%(trailers:key=Harness-Squashed,valueonly)')).toBe(`ticket/x ${tip}`);
      expect(repo.git('status', '--porcelain')).toBe('');
      expect(leftovers()).toBe(false);
    });

    it('legt keinen Commit an, wenn der Branch schon enthalten oder schon zusammengefasst ist', () => {
      repo.git('checkout', '-q', '-b', 'ticket/x');
      repo.git('checkout', '-q', main);
      const head = merger.head(repo.path, 'HEAD');
      expect(merger.merge(repo.path, 'ticket/x', 'm', 'squash')).toEqual({ kind: 'merged', commit: repo.git('rev-parse', '--short', 'HEAD') });
      expect(merger.head(repo.path, 'HEAD')).toBe(head);
      repo.git('checkout', '-q', 'ticket/x');
      commit('b.txt', 'neu');
      repo.git('checkout', '-q', main);
      expect(merger.merge(repo.path, 'ticket/x', 'm', 'squash').kind).toBe('merged');
      const squashCommit = merger.head(repo.path, 'HEAD');
      expect(merger.merge(repo.path, 'ticket/x', 'm', 'squash').kind).toBe('merged');
      expect(merger.merge(repo.path, 'ticket/x', 'm').kind).toBe('merged');
      expect(merger.head(repo.path, 'HEAD')).toBe(squashCommit);
    });
  });

  describe('integrated', () => {
    it('erkennt Vorfahren und zusammengefasste Branches, aber keine neueren Spitzen', () => {
      repo.git('checkout', '-q', '-b', 'ticket/x');
      commit('b.txt', 'neu');
      repo.git('checkout', '-q', main);
      expect(merger.integrated(repo.path, 'ticket/x')).toBe(false);
      merger.merge(repo.path, 'ticket/x', 'm', 'squash');
      expect(merger.integrated(repo.path, 'ticket/x')).toBe(true);
      expect(merger.integrated(repo.path, 'ticket/x', 'HEAD~1')).toBe(false);
      repo.git('checkout', '-q', 'ticket/x');
      commit('c.txt', 'danach');
      repo.git('checkout', '-q', main);
      expect(merger.integrated(repo.path, 'ticket/x')).toBe(false);
      repo.git('branch', 'ticket/y', 'HEAD~1');
      expect(merger.integrated(repo.path, 'ticket/y')).toBe(true);
      expect(merger.integrated(repo.path, 'gibt/es-nicht')).toBe(false);
    });

    it('ein Branch, dessen Name ein Präfix eines anderen ist, gilt nicht als zusammengefasst', () => {
      repo.git('checkout', '-q', '-b', 'ticket/x-2');
      commit('b.txt', 'neu');
      repo.git('branch', 'ticket/x');
      repo.git('checkout', '-q', main);
      merger.merge(repo.path, 'ticket/x-2', 'm', 'squash');
      expect(merger.integrated(repo.path, 'ticket/x-2')).toBe(true);
      expect(merger.integrated(repo.path, 'ticket/x')).toBe(false);
    });

    it('deleteBranch löscht einen zusammengefassten Branch nur auf der geprüften Spitze', () => {
      repo.git('checkout', '-q', '-b', 'ticket/x');
      commit('b.txt', 'neu');
      const tip = repo.git('rev-parse', 'HEAD');
      repo.git('checkout', '-q', main);
      merger.merge(repo.path, 'ticket/x', 'm', 'squash');
      expect(merger.deleteBranch(repo.path, 'ticket/x')).toMatch(/ticket\/x/);
      // Nach der Prüfung kam noch ein Commit dazu: nicht löschen
      repo.git('checkout', '-q', 'ticket/x');
      commit('c.txt', 'später');
      repo.git('checkout', '-q', main);
      expect(merger.deleteBranch(repo.path, 'ticket/x', tip)).toMatch(/ticket\/x/);
      expect(repo.git('branch', '--list', 'ticket/x')).not.toBe('');
      expect(merger.deleteBranch(repo.path, 'ticket/x', repo.git('rev-parse', 'ticket/x'))).toBeNull();
      expect(repo.git('branch', '--list', 'ticket/x')).toBe('');
    });

    it('verweigert den Squash bei vorgemerkten Änderungen (auch beim Vorspulen) und lässt sie unverändert', () => {
      repo.git('checkout', '-q', '-b', 'ticket/x');
      commit('b.txt', 'neu');
      repo.git('checkout', '-q', main);
      const before = repo.git('rev-parse', 'HEAD');
      writeFileSync(join(repo.path, 's.txt'), 'vorgemerkt');
      repo.git('add', 's.txt');
      const outcome = merger.merge(repo.path, 'ticket/x', 'm', 'squash');
      expect(outcome.kind).toBe('refused');
      expect(repo.git('rev-parse', 'HEAD')).toBe(before);
      expect(repo.git('diff', '--cached', '--name-only')).toBe('s.txt');
      expect(readFileSync(join(repo.path, 's.txt'), 'utf8')).toBe('vorgemerkt');
    });

    it('ein scheiternder pre-commit-Hook verhindert den Squash nicht (wie beim Merge-Commit)', () => {
      repo.git('checkout', '-q', '-b', 'ticket/x');
      commit('b.txt', 'neu');
      repo.git('checkout', '-q', main);
      const hooks = join(repo.path, '.git', 'hooks');
      mkdirSync(hooks, { recursive: true });
      writeFileSync(join(hooks, 'pre-commit'), '#!/bin/sh\nexit 1\n');
      chmodSync(join(hooks, 'pre-commit'), 0o755);
      expect(merger.merge(repo.path, 'ticket/x', 'm', 'squash').kind).toBe('merged');
      expect(merger.integrated(repo.path, 'ticket/x')).toBe(true);
    });
  });
});

// GitRunner, der ausgewählte Aufrufe scheitern lässt und alle Aufrufe mitschreibt
class FakeGit extends GitRunner {
  readonly calls: string[][] = [];

  constructor(private readonly failing: (args: string[]) => GitAttempt | null) {
    super();
  }

  override raw(cwd: string, args: string[]): string {
    this.calls.push(args);
    const failed = this.failing(args);
    if (failed) throw new HarnessError('git-failed', failed.err);
    return super.raw(cwd, args);
  }

  override tryRun(cwd: string, args: string[]): GitAttempt {
    this.calls.push(args);
    return this.failing(args) ?? super.tryRun(cwd, args);
  }
}
