import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TempRepo } from '../helpers/TempRepo.js';
import { GitRunner } from '../../../src/git/GitRunner.js';

describe('GitRunner', () => {
  let repo: TempRepo;
  const git = new GitRunner();

  beforeEach(() => {
    repo = new TempRepo();
  });
  afterEach(() => repo.dispose());

  it('tryRun füllt die Fehlermeldung auch, wenn Git nichts auf stderr schreibt', () => {
    repo.git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '--allow-empty', '-q', '-m', 'zwei');
    // HEAD ist kein Vorfahr von HEAD~1: Exit-Code 1 ohne stderr
    const attempt = git.tryRun(repo.path, ['merge-base', '--is-ancestor', 'HEAD', 'HEAD~1']);
    expect(attempt.ok).toBe(false);
    expect(attempt.err).not.toBe('');
  });
});
