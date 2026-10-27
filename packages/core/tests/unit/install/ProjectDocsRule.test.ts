import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { TempRepo } from '../helpers/TempRepo.js';
import { HarnessHome } from '../../../src/registry/HarnessHome.js';
import { ProjectDocsRule } from '../../../src/install/ProjectDocsRule.js';

describe('ProjectDocsRule', () => {
  let repo: TempRepo;
  const rule = new ProjectDocsRule(new HarnessHome({}));
  const read = (dir: string) => readFileSync(rule.file(dir), 'utf8');

  beforeEach(() => {
    repo = new TempRepo();
  });

  afterEach(() => repo.dispose());

  it('legt die Rule mit Eingrenzung auf docs/ und Import der zentralen Regeln an', async () => {
    expect(await rule.ensure(repo.path)).toBe(true);
    expect(rule.file(repo.path)).toBe(join(repo.path, '.claude', 'rules', 'ai-harness-docs.md'));
    const text = read(repo.path);
    expect(text).toMatch(/^---\n/);
    expect(text).toContain('paths:\n  - "**/docs/**"\n');
    expect(text).toMatch(/\n---\n@~\/\.harness\/rules\/docs\.md\n$/);
  });

  it('ist wiederholbar und meldet false, wenn die Rule schon aktuell ist', async () => {
    await rule.ensure(repo.path);
    expect(await rule.ensure(repo.path)).toBe(false);
  });

  it('ersetzt eine veraltete Rule', async () => {
    await rule.ensure(repo.path);
    writeFileSync(rule.file(repo.path), 'alt');
    expect(await rule.ensure(repo.path)).toBe(true);
    expect(read(repo.path)).toContain('@~/.harness/rules/docs.md');
  });

  it('nimmt die Rule aus Git heraus, ohne eine versionierte Datei zu ändern', async () => {
    await rule.ensure(repo.path);
    expect(repo.git('status', '--porcelain', '--untracked-files=all')).toBe('');
    const exclude = readFileSync(join(repo.git('rev-parse', '--path-format=absolute', '--git-common-dir'), 'info', 'exclude'), 'utf8');
    expect(exclude).toContain('/.claude/rules/ai-harness-docs.md');
  });

  it('gilt auch in einem Worktree; der Ausschluss liegt im gemeinsamen Git-Verzeichnis', async () => {
    const worktree = repo.addWorktree('wt');
    expect(await rule.ensure(worktree)).toBe(true);
    expect(existsSync(rule.file(worktree))).toBe(true);
    expect(repo.git('-C', worktree, 'status', '--porcelain', '--untracked-files=all')).toBe('');
  });

  it('importiert bei eigenem HARNESS_HOME den absoluten Pfad, Leerzeichen maskiert', () => {
    const custom = new ProjectDocsRule(new HarnessHome({ HARNESS_HOME: 'C:\\Users\\Max Muster\\.harness' }));
    expect(custom.importTarget()).toBe('C:/Users/Max\\ Muster/.harness/rules/docs.md');
  });
});
