import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { existsSync, rmSync } from 'node:fs';
import { TempRepo } from '../helpers/TempRepo.js';
import { HarnessHome } from '../../../src/registry/HarnessHome.js';
import { PROJECT_RULE_PATHS, ProjectRules } from '../../../src/install/ProjectRules.js';

describe('ProjectRules', () => {
  let repo: TempRepo;
  const rules = new ProjectRules(new HarnessHome({}));

  beforeEach(() => {
    repo = new TempRepo();
  });

  afterEach(() => repo.dispose());

  it('kennt die Orte beider Rules unter altem und neuem Namen', () => {
    expect(PROJECT_RULE_PATHS).toEqual([
      '.claude/rules/ai-harness-docs.md',
      '.claude/rules/ai-harness-board.md',
      '.claude/rules/nexus-docs.md',
      '.claude/rules/nexus-board.md',
    ]);
    expect(rules.all().map((r) => r.path)).toEqual(PROJECT_RULE_PATHS.slice(0, 2));
  });

  it('schreibt beide Rules und meldet bei Wiederholung nichts Neues', async () => {
    expect(await rules.ensure(repo.path)).toEqual([rules.docs, rules.board]);
    expect(await rules.ensure(repo.path)).toEqual([]);
    expect(repo.git('status', '--porcelain', '--untracked-files=all')).toBe('');
  });

  it('ergänzt in einem schon eingerichteten Projekt nur die fehlende Board-Rule', async () => {
    await rules.ensure(repo.path);
    rmSync(rules.board.file(repo.path));
    expect(await rules.ensure(repo.path)).toEqual([rules.board]);
    expect(existsSync(rules.board.file(repo.path))).toBe(true);
  });
});
