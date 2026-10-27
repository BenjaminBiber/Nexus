import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TempRepo } from '../helpers/TempRepo.js';
import { BranchCommitReader } from '../../../src/git/BranchCommitReader.js';

const ID = ['-c', 'user.name=Tester', '-c', 'user.email=t@t'];

describe('BranchCommitReader', () => {
  let repo: TempRepo;
  const reader = new BranchCommitReader();

  beforeEach(() => {
    repo = new TempRepo();
    repo.git('branch', '-M', 'main');
    repo.git('checkout', '-q', '-b', 'ticket/ARC-0001-x');
    for (const name of ['a', 'b']) {
      writeFileSync(join(repo.path, `${name}.txt`), name);
      repo.git('add', '.');
      repo.git(...ID, 'commit', '-q', '-m', `Commit ${name}`);
    }
    repo.git('checkout', '-q', 'main');
  });
  afterEach(() => repo.dispose());

  it('liest die Commits des Branches vor dem Merge (neueste zuerst)', () => {
    const result = reader.read(repo.path, { branch: 'ticket/ARC-0001-x', mergeCommit: null });
    expect(result.status).toBe('ok');
    expect(result.commits.map((c) => c.subject)).toEqual(['Commit b', 'Commit a']);
    expect(result.commits[0]).toMatchObject({ author: 'Tester', hash: expect.stringMatching(/^[0-9a-f]{40}$/) });
    expect(result.commits[0].at).toMatch(/Z$/);
  });

  it('liest die Commits nach einem Merge-Commit über dessen Eltern', () => {
    repo.git(...ID, 'merge', '--no-ff', '-q', '-m', 'Merge', 'ticket/ARC-0001-x');
    const hash = repo.git('rev-parse', '--short', 'HEAD');
    repo.git('branch', '-D', 'ticket/ARC-0001-x');
    const result = reader.read(repo.path, { branch: 'ticket/ARC-0001-x', mergeCommit: hash });
    expect(result.status).toBe('ok');
    expect(result.commits.map((c) => c.subject)).toEqual(['Commit b', 'Commit a']);
  });

  it('liefert bei Squash nur den Squash-Commit', () => {
    repo.git('merge', '--squash', '-q', 'ticket/ARC-0001-x');
    repo.git(...ID, 'commit', '-q', '-m', 'Squash ARC-0001');
    const hash = repo.git('rev-parse', 'HEAD');
    const result = reader.read(repo.path, { branch: null, mergeCommit: hash });
    expect(result.commits.map((c) => c.subject)).toEqual(['Squash ARC-0001']);
  });

  it('meldet fehlenden Branch, fehlendes Repo und ungültige Angaben als unavailable ohne Ausnahme', () => {
    expect(reader.read(repo.path, { branch: 'gibt-es-nicht', mergeCommit: null }).status).toBe('unavailable');
    expect(reader.read(repo.path, { branch: null, mergeCommit: null }).status).toBe('unavailable');
    expect(reader.read(join(repo.base, 'fehlt'), { branch: 'ticket/ARC-0001-x', mergeCommit: null }).status).toBe('unavailable');
    expect(reader.read(null, { branch: 'ticket/ARC-0001-x', mergeCommit: null }).status).toBe('unavailable');
    expect(reader.read(repo.path, { branch: '--output=x', mergeCommit: '--all' }).status).toBe('unavailable');
  });

  it('begrenzt auf die angegebene Zahl', () => {
    expect(new BranchCommitReader(1).read(repo.path, { branch: 'ticket/ARC-0001-x', mergeCommit: null }).commits).toHaveLength(1);
  });

  describe('mit Zielbranch', () => {
    // release/1 von main mit Commit r, Ticket-Branch ticket/ARC-0002-y von release/1 mit Commit t; ausgecheckt bleibt main
    beforeEach(() => {
      repo.git('checkout', '-q', '-b', 'release/1');
      writeFileSync(join(repo.path, 'r.txt'), 'r');
      repo.git('add', '.');
      repo.git(...ID, 'commit', '-q', '-m', 'Commit r');
      repo.git('checkout', '-q', '-b', 'ticket/ARC-0002-y');
      writeFileSync(join(repo.path, 't.txt'), 't');
      repo.git('add', '.');
      repo.git(...ID, 'commit', '-q', '-m', 'Commit t');
      repo.git('checkout', '-q', 'main');
    });

    it('liest vor dem Merge nur die Commits seit dem Zielbranch', () => {
      const result = reader.read(repo.path, { branch: 'ticket/ARC-0002-y', mergeCommit: null, targetBranch: 'release/1' });
      expect(result.status).toBe('ok');
      expect(result.commits.map((c) => c.subject)).toEqual(['Commit t']);
    });

    it('zählt ohne Zielbranch ab dem ausgecheckten Branch', () => {
      for (const targetBranch of [undefined, null]) {
        const result = reader.read(repo.path, { branch: 'ticket/ARC-0002-y', mergeCommit: null, targetBranch });
        expect(result.commits.map((c) => c.subject)).toEqual(['Commit t', 'Commit r']);
      }
    });

    it('fällt bei nicht vorhandenem oder ungültigem Zielbranch auf HEAD zurück', () => {
      for (const targetBranch of ['release/2', '--output=x']) {
        const result = reader.read(repo.path, { branch: 'ticket/ARC-0002-y', mergeCommit: null, targetBranch });
        expect(result.status).toBe('ok');
        expect(result.commits.map((c) => c.subject)).toEqual(['Commit t', 'Commit r']);
      }
    });

    it('liest nach dem Merge weiter über den Merge-Commit, auch mit Zielbranch', () => {
      repo.git('checkout', '-q', 'release/1');
      repo.git(...ID, 'merge', '--no-ff', '-q', '-m', 'Merge', 'ticket/ARC-0002-y');
      const hash = repo.git('rev-parse', 'HEAD');
      repo.git('checkout', '-q', 'main');
      repo.git('branch', '-D', 'ticket/ARC-0002-y');
      const result = reader.read(repo.path, { branch: 'ticket/ARC-0002-y', mergeCommit: hash, targetBranch: 'release/1' });
      expect(result.commits.map((c) => c.subject)).toEqual(['Commit t']);
    });
  });
});
