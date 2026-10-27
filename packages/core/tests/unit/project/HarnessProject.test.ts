import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { TempRepo } from '../helpers/TempRepo.js';
import { HarnessProject } from '../../../src/project/HarnessProject.js';
import { HarnessError } from '../../../src/errors/HarnessError.js';

describe('HarnessProject', () => {
  let repo: TempRepo;
  beforeEach(() => (repo = new TempRepo()));
  afterEach(() => repo.dispose());

  it('legt Zähler und Token an, aber keinen Ticket-Ordner', async () => {
    const project = await HarnessProject.init(repo.path);
    expect(existsSync(join(project.layout.dir, 'tickets'))).toBe(false);
    expect(JSON.parse(readFileSync(project.layout.countersFile, 'utf8'))).toEqual({ session: 0 });
    expect(readFileSync(project.layout.tokenFile, 'utf8')).toMatch(/^[0-9a-f]{64}$/);
  });

  it('liegt im gemeinsamen Git-Verzeichnis', async () => {
    const project = await HarnessProject.init(repo.path);
    expect(project.layout.dir.replace(/\\/g, '/')).toContain('/.git/harness');
  });

  it('ändert bei zweitem init weder Token noch Zähler', async () => {
    const first = await HarnessProject.init(repo.path);
    await first.counter.next('session');
    const token = readFileSync(first.layout.tokenFile, 'utf8');
    const second = await HarnessProject.init(repo.path);
    expect(readFileSync(second.layout.tokenFile, 'utf8')).toBe(token);
    expect(JSON.parse(readFileSync(second.layout.countersFile, 'utf8')).session).toBe(1);
  });

  it('lässt alte Zählerstände des Datei-Boards und des Spec-Kits in der Datei stehen', async () => {
    const project = await HarnessProject.init(repo.path);
    writeFileSync(project.layout.countersFile, JSON.stringify({ ticket: 42, spec: 3, question: 7 }));
    expect(await project.counter.next('session')).toBe(1);
    expect(JSON.parse(readFileSync(project.layout.countersFile, 'utf8'))).toEqual({ ticket: 42, spec: 3, question: 7, session: 1 });
  });

  it('open ohne init wirft einen Hinweis auf harness init', async () => {
    await expect(HarnessProject.open(repo.path)).rejects.toThrow(HarnessError);
    await expect(HarnessProject.open(repo.path)).rejects.toThrow(/harness init/);
  });

  it('nutzt aus einem Worktree dasselbe Verzeichnis', async () => {
    const main = await HarnessProject.init(repo.path);
    const fromWorktree = await HarnessProject.open(repo.addWorktree('wt'));
    expect(fromWorktree.layout.dir).toBe(main.layout.dir);
  });

  it('meldet einen beschädigten Zähler mit Pfad, statt neu zu beginnen', async () => {
    const project = await HarnessProject.init(repo.path);
    writeFileSync(project.layout.countersFile, 'x');
    const reopened = await HarnessProject.open(repo.path);
    await expect(reopened.counter.next('session')).rejects.toThrow(/beschädigt/);
  });
});
