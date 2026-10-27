import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TempRepo } from '../helpers/TempRepo.js';
import { BranchNames } from '../../../src/git/BranchNames.js';

describe('BranchNames', () => {
  let repo: TempRepo;
  const names = new BranchNames();

  beforeEach(() => {
    repo = new TempRepo();
    repo.git('branch', '-M', 'main');
    repo.git('branch', 'release/1');
    repo.git('branch', 'ticket/ARC-0001-x');
  });
  afterEach(() => repo.dispose());

  it('liefert die lokalen Branches alphabetisch ohne den Namensraum der Tickets', () => {
    expect(names.list(repo.path)).toEqual(['main', 'release/1']);
  });

  it('führt die Branches von origin ohne Präfix mit den lokalen zusammen', () => {
    // Remote-Refs direkt setzen: ein echtes origin braucht es dafür nicht
    repo.git('update-ref', 'refs/remotes/origin/main', 'HEAD');
    repo.git('update-ref', 'refs/remotes/origin/HEAD', 'HEAD');
    repo.git('update-ref', 'refs/remotes/origin/feature/neu', 'HEAD');
    expect(names.list(repo.path)).toEqual(['feature/neu', 'main', 'release/1']);
  });

  it('liefert ohne Repo eine leere Liste', () => {
    expect(names.list(join(repo.base, 'fehlt'))).toEqual([]);
  });

  it('begrenzt die Zahl der Namen', () => {
    expect(new BranchNames(1).list(repo.path)).toEqual(['main']);
  });
});
