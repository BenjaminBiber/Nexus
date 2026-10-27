import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { TempRepo } from '../helpers/TempRepo.js';
import { HarnessHome } from '../../../src/registry/HarnessHome.js';
import { ProjectBoardRule } from '../../../src/install/ProjectBoardRule.js';

describe('ProjectBoardRule', () => {
  let repo: TempRepo;
  const rule = new ProjectBoardRule(new HarnessHome({}));

  beforeEach(() => {
    repo = new TempRepo();
  });

  afterEach(() => repo.dispose());

  it('legt die Rule ohne Eingrenzung an (immer geladen) und importiert die zentrale Board-Regel', async () => {
    expect(await rule.ensure(repo.path)).toBe(true);
    expect(rule.file(repo.path)).toBe(join(repo.path, '.claude', 'rules', 'ai-harness-board.md'));
    const text = readFileSync(rule.file(repo.path), 'utf8');
    expect(text).not.toContain('paths:');
    expect(text).toMatch(/^---\n# .*\n---\n@~\/\.harness\/rules\/board\.md\n$/);
  });

  it('ist wiederholbar und nimmt die Rule aus Git heraus', async () => {
    await rule.ensure(repo.path);
    expect(await rule.ensure(repo.path)).toBe(false);
    expect(repo.git('status', '--porcelain', '--untracked-files=all')).toBe('');
    const exclude = readFileSync(join(repo.git('rev-parse', '--path-format=absolute', '--git-common-dir'), 'info', 'exclude'), 'utf8');
    expect(exclude).toContain('/.claude/rules/ai-harness-board.md');
  });

  it('importiert bei eigenem HARNESS_HOME den absoluten Pfad, Leerzeichen maskiert', () => {
    const custom = new ProjectBoardRule(new HarnessHome({ HARNESS_HOME: 'C:\\Users\\Max Muster\\.harness' }));
    expect(custom.importTarget()).toBe('C:/Users/Max\\ Muster/.harness/rules/board.md');
  });
});
