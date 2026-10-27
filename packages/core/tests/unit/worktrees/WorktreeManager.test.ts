import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { join } from 'node:path';
import { TempRepo } from '../helpers/TempRepo.js';
import { HarnessProject } from '../../../src/project/HarnessProject.js';
import { WorktreeManager } from '../../../src/worktrees/WorktreeManager.js';
import { TargetBranches } from '../../../src/dispatch/git/TargetBranches.js';
import { BoardServices } from '../../../src/board/BoardServices.js';
import { BoardFixture } from '../board/helpers/BoardFixture.js';
import type { NewTicketInput } from '../../../src/board/TicketService.js';

describe('WorktreeManager (DB-Board)', () => {
  let repo: TempRepo;
  let f: BoardFixture;
  let manager: WorktreeManager;
  let layout: HarnessProject['layout'];

  // Legt ein Ticket auf dem Board der Testumgebung an
  const ticket = (input: NewTicketInput) => f.tickets.create(f.boardId, input, f.actor);

  beforeEach(async () => {
    repo = new TempRepo();
    const project = await HarnessProject.init(repo.path);
    layout = project.layout;
    f = await BoardFixture.create();
    manager = new WorktreeManager(project.layout, new BoardServices(f.db, f.clock, f.repoPaths), project.root);
  });
  afterEach(() => repo.dispose());

  it('legt Branch und Worktree an, setzt In Arbeit und schaltet das Plugin im Worktree ein', async () => {
    await ticket({ title: 'Login bauen', touches: ['src/login'] });
    const result = await manager.start('ARC-0001', {}, repo.path, f.actor);
    expect(result).toMatchObject({ id: 'ARC-0001', branch: 'ticket/ARC-0001-login-bauen', joined: false });
    expect(existsSync(result.path)).toBe(true);
    expect(existsSync(join(result.path, '.claude', 'settings.local.json'))).toBe(true);
    expect(repo.git('branch', '--list')).toContain('ticket/ARC-0001-login-bauen');
    expect(await f.tickets.get('ARC-0001')).toMatchObject({ status: 'in_progress', branch: result.branch, worktree: result.path });
    expect((await manager.registry.read())[result.branch].tickets).toEqual(['ARC-0001']);
  });

  it('verknüpft die Session, wenn eine angegeben ist', async () => {
    await ticket({ title: 'A' });
    await manager.start('ARC-0001', { sessionId: 's-1' }, repo.path, f.actor);
    expect((await f.tickets.get('ARC-0001')).sessions).toEqual(['s-1']);
  });

  it('tritt mit --in einem Worktree ohne Überschneidung bei, ohne neuen Branch', async () => {
    await ticket({ title: 'Erstes', touches: ['src/a'] });
    await ticket({ title: 'Zweites', touches: ['src/b'] });
    const first = await manager.start('ARC-0001', {}, repo.path, f.actor);
    const second = await manager.start('ARC-0002', { inTicket: 'ARC-0001' }, repo.path, f.actor);
    expect(second).toMatchObject({ joined: true, branch: first.branch, path: first.path });
    expect(repo.git('branch', '--list', 'ticket/*').split('\n')).toHaveLength(1);
    expect((await manager.registry.read())[first.branch].tickets).toEqual(['ARC-0001', 'ARC-0002']);
  });

  it('tritt mit join automatisch nur einem offenen Worktree derselben Spec ohne Überschneidung bei', async () => {
    await ticket({ title: 'Erstes', touches: ['a'], spec: '001-x', specTask: 'T001' });
    await ticket({ title: 'Anderes', touches: ['b'], spec: '002-y', specTask: 'T001' });
    await ticket({ title: 'Gleiches', touches: ['c'], spec: '001-x', specTask: 'T002' });
    await ticket({ title: 'Überschneidet', touches: ['a/x'], spec: '001-x', specTask: 'T003' });
    const first = await manager.start('ARC-0001', { join: true }, repo.path, f.actor);
    expect((await manager.start('ARC-0002', { join: true }, repo.path, f.actor)).joined).toBe(false);
    expect(await manager.start('ARC-0003', { join: true }, repo.path, f.actor)).toMatchObject({ joined: true, branch: first.branch });
    const overlap = await manager.start('ARC-0004', { join: true }, repo.path, f.actor);
    expect(overlap.joined).toBe(false);
    expect(overlap.branch).not.toBe(first.branch);
  });

  it('ohne join tritt start keinem Worktree derselben Spec bei, jedes Ticket bekommt einen eigenen Branch', async () => {
    await ticket({ title: 'Erstes', touches: ['a'], spec: '001-x', specTask: 'T001' });
    await ticket({ title: 'Gleiches', touches: ['b'], spec: '001-x', specTask: 'T002' });
    const first = await manager.start('ARC-0001', {}, repo.path, f.actor);
    const second = await manager.start('ARC-0002', {}, repo.path, f.actor);
    expect(second).toMatchObject({ joined: false, branch: 'ticket/ARC-0002-gleiches' });
    expect(second.path).not.toBe(first.path);
    expect(await manager.ticketsOf(first.branch)).toEqual(['ARC-0001']);
  });

  it('warnt bei Überschneidung mit --in und legt einen eigenen Worktree an', async () => {
    await ticket({ title: 'Erstes', touches: ['src'] });
    await ticket({ title: 'Zweites', touches: ['src/login'] });
    const first = await manager.start('ARC-0001', {}, repo.path, f.actor);
    const second = await manager.start('ARC-0002', { inTicket: 'ARC-0001' }, repo.path, f.actor);
    expect(second.joined).toBe(false);
    expect(second.warning).toMatch(/überschneid/i);
    expect(second.path).not.toBe(first.path);
  });

  it('beachtet die Sperren vor dem Anlegen; --force übergeht eine offene Voraussetzung', async () => {
    await ticket({ title: 'Basis' });
    await ticket({ title: 'Folge', dependsOn: ['ARC-0001'] });
    await expect(manager.start('ARC-0002', {}, repo.path, f.actor)).rejects.toMatchObject({ code: 'dependency-open' });
    expect(repo.git('branch', '--list', 'ticket/*')).toBe('');
    const forced = await manager.start('ARC-0002', { force: true }, repo.path, f.actor);
    expect((await f.tickets.get('ARC-0002')).status).toBe('in_progress');
    expect(existsSync(forced.path)).toBe(true);
  });

  it('meldet gestartete Tickets, fehlende Ziel-Worktrees, vorhandene Branches und den fehlenden Commit', async () => {
    await ticket({ title: 'Erstes' });
    await ticket({ title: 'Zweites' });
    await expect(manager.start('ARC-0002', { inTicket: 'ARC-0001' }, repo.path, f.actor)).rejects.toMatchObject({ code: 'host-not-started' });
    repo.git('branch', 'ticket/ARC-0002-zweites');
    await expect(manager.start('ARC-0002', {}, repo.path, f.actor)).rejects.toMatchObject({ code: 'branch-exists' });
    await manager.start('ARC-0001', {}, repo.path, f.actor);
    await expect(manager.start('ARC-0001', {}, repo.path, f.actor)).rejects.toMatchObject({ code: 'already-started' });

    const empty = new TempRepo(false);
    try {
      const p = await HarnessProject.init(empty.path);
      const m = new WorktreeManager(p.layout, new BoardServices(f.db, f.clock, f.repoPaths), p.root);
      await ticket({ title: 'Drittes' });
      await expect(m.start('ARC-0003', {}, empty.path, f.actor)).rejects.toMatchObject({ code: 'no-commit' });
    } finally {
      empty.dispose();
    }
  });

  it('done verlangt eine Abnahme, setzt Erledigt und meldet „bereit“ erst, wenn alle fertig sind', async () => {
    await ticket({ title: 'Erstes', touches: ['a'] });
    await ticket({ title: 'Zweites', touches: ['b'] });
    const first = await manager.start('ARC-0001', {}, repo.path, f.actor);
    await manager.start('ARC-0002', { inTicket: 'ARC-0001' }, repo.path, f.actor);

    await expect(manager.finish('ARC-0001', {}, repo.path, f.actor)).rejects.toMatchObject({ code: 'acceptance-missing' });
    const one = await manager.finish('ARC-0001', { acceptance: 'Test grün' }, repo.path, f.actor);
    expect(one).toMatchObject({ id: 'ARC-0001', readyToMerge: false, warnings: [] });
    expect(await f.tickets.get('ARC-0001')).toMatchObject({ status: 'done', acceptance: 'Test grün' });
    const two = await manager.finish('ARC-0002', { acceptance: 'ok' }, first.path, f.actor);
    expect(two.readyToMerge).toBe(true);
    expect((await manager.registry.read())[first.branch].readyToMerge).toBe(true);
  });

  it('done lässt den Ticket-Branch unberührt: kein zusätzlicher Commit, nichts im Index', async () => {
    repo.git('config', 'user.name', 't');
    repo.git('config', 'user.email', 't@t');
    await ticket({ title: 'Erstes', touches: ['a'], spec: 'docs/superpowers/specs/2026-10-22-x-design.md', acceptance: 'ok' });
    const started = await manager.start('ARC-0001', {}, repo.path, f.actor);
    const head = repo.git('rev-parse', started.branch);

    const result = await manager.finish('ARC-0001', {}, started.path, f.actor);
    expect(result).toMatchObject({ readyToMerge: true, warnings: [] });
    expect(repo.git('rev-parse', started.branch)).toBe(head);
    expect(repo.git('-C', started.path, 'diff', '--cached', '--name-only')).toBe('');
    expect((await f.tickets.get('ARC-0001')).status).toBe('done');
  });

  it('gebündelt (sameSpec): hält den Worktree offen, solange ein Ticket derselben Spec noch auf den Start wartet', async () => {
    const bundled = new WorktreeManager(layout, new BoardServices(f.db, f.clock, f.repoPaths), repo.path, { joinWorktrees: 'sameSpec' });
    await ticket({ title: 'Erstes', touches: ['a'], spec: '001-x', specTask: 'T001' });
    await ticket({ title: 'Zweites', touches: ['a'], spec: '001-x', specTask: 'T002' });
    const first = await bundled.start('ARC-0001', { join: true }, repo.path, f.actor);
    expect(await bundled.finish('ARC-0001', { acceptance: 'ok' }, repo.path, f.actor)).toMatchObject({ readyToMerge: false, removed: [] });
    expect(existsSync(first.path)).toBe(true);
    expect(await bundled.isReady(first.branch)).toBe(false);
    expect(await bundled.start('ARC-0002', { join: true }, repo.path, f.actor)).toMatchObject({ joined: true, branch: first.branch });
    expect((await bundled.finish('ARC-0002', { acceptance: 'ok' }, first.path, f.actor)).readyToMerge).toBe(true);
  });

  it('ein Worktree je Ticket: bereit zum Merge, sobald das Ticket fertig ist, auch wenn Tickets derselben Spec noch warten', async () => {
    await ticket({ title: 'Erstes', touches: ['a'], spec: '001-x', specTask: 'T001' });
    await ticket({ title: 'Zweites', touches: ['b'], spec: '001-x', specTask: 'T002' });
    const first = await manager.start('ARC-0001', {}, repo.path, f.actor);
    expect(await manager.finish('ARC-0001', { acceptance: 'ok' }, repo.path, f.actor)).toMatchObject({ readyToMerge: true });
    expect(await manager.isReady(first.branch)).toBe(true);
  });

  it('ein Worktree je Ticket: isReady prüft jetzt, auch wenn das Ticket von Hand auf Erledigt kam', async () => {
    await ticket({ title: 'Erstes', acceptance: 'ok' });
    const first = await manager.start('ARC-0001', {}, repo.path, f.actor);
    expect(await manager.isReady(first.branch)).toBe(false);
    await f.tickets.move('ARC-0001', 'done', f.actor);
    expect(await manager.isReady(first.branch)).toBe(true);
    expect((await manager.registry.read())[first.branch].readyToMerge).toBe(true);
  });

  it('ein vom Merge blockiertes Ticket („Merge: …“) gilt als fertig, damit „Erneut mergen“ nicht ewig wartet', async () => {
    await ticket({ title: 'Erstes', acceptance: 'ok' });
    const first = await manager.start('ARC-0001', {}, repo.path, f.actor);
    await f.tickets.update('ARC-0001', { status: 'blocked', blockedReason: 'Merge: lokale Änderungen im Hauptordner: a.txt' }, f.actor);
    expect(await manager.isReady(first.branch)).toBe(true);
    await f.tickets.update('ARC-0001', { status: 'blocked', blockedReason: 'Human Interaction: ansehen' }, f.actor);
    expect(await manager.isReady(first.branch)).toBe(false);
  });

  it('accept: nimmt ein Human-Interaction-Ticket ab und meldet den Worktree bereit', async () => {
    await ticket({ title: 'Ansehen', acceptance: 'Oberfläche geprüft' });
    const started = await manager.start('ARC-0001', {}, repo.path, f.actor);
    await expect(manager.accept('ARC-0001', {}, repo.path, f.actor)).rejects.toMatchObject({ code: 'accept-not-possible' });
    await f.tickets.update('ARC-0001', { status: 'blocked', blockedReason: 'Human Interaction: bitte ansehen' }, f.actor);
    const result = await manager.accept('ARC-0001', {}, repo.path, f.actor);
    expect(result).toMatchObject({ id: 'ARC-0001', branch: started.branch, readyToMerge: true, warnings: [] });
    expect(await f.tickets.get('ARC-0001')).toMatchObject({ status: 'done', blockedReason: null, branch: started.branch });
    // Der saubere, fertige Worktree ist danach aufgeräumt
    expect(result.removed).toEqual([started.path]);
  });

  it('räumt fertige, saubere Worktrees außerhalb des aktuellen Ordners auf, nie mit ungesicherten Änderungen, ohne zu mergen', async () => {
    await ticket({ title: 'Erstes' });
    await ticket({ title: 'Zweites' });
    await ticket({ title: 'Drittes' });
    const first = await manager.start('ARC-0001', {}, repo.path, f.actor);
    const head = repo.git('rev-parse', 'HEAD');
    const done = await manager.finish('ARC-0001', { acceptance: 'ok' }, repo.path, f.actor);
    expect(done.removed).toEqual([first.path]);
    expect(existsSync(first.path)).toBe(false);
    expect(repo.git('branch', '--list')).toContain(first.branch);
    expect(repo.git('rev-parse', 'HEAD')).toBe(head);

    const second = await manager.start('ARC-0002', {}, repo.path, f.actor);
    writeFileSync(join(second.path, 'neu.txt'), 'wichtig');
    expect((await manager.finish('ARC-0002', { acceptance: 'ok' }, repo.path, f.actor)).removed).toEqual([]);
    expect(existsSync(join(second.path, 'neu.txt'))).toBe(true);
  });

  it('legt im neuen Worktree die Rules für Doku- und Board-Regeln an', async () => {
    await ticket({ title: 'Erstes' });
    const first = await manager.start('ARC-0001', {}, repo.path, f.actor);
    expect(existsSync(join(first.path, '.claude', 'rules', 'ai-harness-docs.md'))).toBe(true);
    expect(readFileSync(join(first.path, '.claude', 'rules', 'ai-harness-board.md'), 'utf8')).toMatch(/\n@.*rules\/board\.md\n$/);
    expect(repo.git('-C', first.path, 'status', '--porcelain', '--untracked-files=all')).toBe('');
  });

  it('gibt im neuen Worktree den Import der Doku-Regeln frei', async () => {
    const configFile = join(process.env.CLAUDE_CONFIG_DIR as string, '.claude.json');
    writeFileSync(configFile, JSON.stringify({ projects: {} }));
    await ticket({ title: 'Erstes' });
    const first = await manager.start('ARC-0001', {}, repo.path, f.actor);
    const projects = JSON.parse(readFileSync(configFile, 'utf8')).projects as Record<string, Record<string, unknown>>;
    expect(projects[first.path]?.hasClaudeMdExternalIncludesApproved).toBe(true);
  });

  it('räumt einen Worktree auch auf, wenn die Rule nicht von Git ausgenommen ist', async () => {
    await ticket({ title: 'Erstes' });
    const first = await manager.start('ARC-0001', {}, repo.path, f.actor);
    // Projekt aus der Zeit vor der Rule: kein Eintrag in info/exclude
    const exclude = join(repo.git('rev-parse', '--path-format=absolute', '--git-common-dir'), 'info', 'exclude');
    writeFileSync(exclude, '');
    const done = await manager.finish('ARC-0001', { acceptance: 'ok' }, repo.path, f.actor);
    expect(done.removed).toEqual([first.path]);
  });

  it('done eines nie gestarteten Tickets meldet keine Warnungen und keinen Branch', async () => {
    await ticket({ title: 'Erstes', spec: 'docs/superpowers/specs/2026-10-22-x-design.md', acceptance: 'steht schon' });
    const result = await manager.finish('ARC-0001', {}, repo.path, f.actor);
    expect(result.warnings).toEqual([]);
    expect(result.branch).toBeNull();
    expect((await f.tickets.get('ARC-0001')).status).toBe('done');
  });

  it('lehnt start und done ab, wenn das Ticket auf einem anderen Board liegt als dem des Repos', async () => {
    await ticket({ title: 'Fremd', acceptance: 'ok' });
    const other = await f.boards.create('ARC', 'Anderes', f.actor);
    const repoBoard = { id: other.id, name: other.name };
    await expect(manager.start('ARC-0001', { repoBoard }, repo.path, f.actor)).rejects.toMatchObject({
      code: 'board-repo-mismatch',
      message: expect.stringMatching(new RegExp(`Board ${f.boardId} .*Sprint.*Board ${other.id} .*Anderes.*harness board boards repo`, 's')),
    });
    expect(repo.git('branch', '--list', 'ticket/*')).toBe('');
    await expect(manager.finish('ARC-0001', { repoBoard }, repo.path, f.actor)).rejects.toMatchObject({ code: 'board-repo-mismatch' });
    expect((await f.tickets.get('ARC-0001')).status).not.toBe('done');
    const own = { id: f.boardId, name: 'Sprint' };
    expect((await manager.start('ARC-0001', { repoBoard: own }, repo.path, f.actor)).joined).toBe(false);
    expect((await manager.finish('ARC-0001', { repoBoard: own }, repo.path, f.actor)).id).toBe('ARC-0001');
  });

  describe('Zielbranch', () => {
    // Git in einem beliebigen Ordner, Commits mit fester Identität
    const execIn = (cwd: string, ...args: string[]): string =>
      execFileSync('git', ['-c', 'user.name=t', '-c', 'user.email=t@t', ...args], { cwd, encoding: 'utf8', windowsHide: true }).trim();

    it('zweigt vom Zielbranch ab und legt ihn bei Bedarf an', async () => {
      await ticket({ title: 'A', targetBranch: 'release/1' });
      const r = await manager.start('ARC-0001', {}, repo.path, f.actor);
      expect(r.targetCreated).toMatch(/^Zielbranch release\/1 neu angelegt von \S+\.$/);
      execIn(repo.path, 'commit', '--allow-empty', '-q', '-m', 'nur auf main');
      await ticket({ title: 'B', targetBranch: 'release/1' });
      const r2 = await manager.start('ARC-0002', {}, repo.path, f.actor);
      expect(r2.targetCreated).toBeUndefined();
      // Ticket-Branch steht auf release/1, der spätere Commit des Hauptordners fehlt darin
      expect(repo.git('rev-parse', r2.branch)).toBe(repo.git('rev-parse', 'release/1'));
      expect(() => repo.git('merge-base', '--is-ancestor', 'HEAD', r2.branch)).toThrow();
    });

    it('legt einen nur als origin/<name> vorhandenen Zielbranch von dort an und nennt den Startpunkt', async () => {
      execIn(repo.path, 'commit', '--allow-empty', '-q', '-m', 'remote');
      repo.git('update-ref', 'refs/remotes/origin/release/2', 'HEAD');
      execIn(repo.path, 'reset', '-q', '--hard', 'HEAD~1');
      await ticket({ title: 'A', targetBranch: 'release/2' });
      const r = await manager.start('ARC-0001', {}, repo.path, f.actor);
      expect(r.targetCreated).toBe('Zielbranch release/2 neu angelegt von origin/release/2.');
      expect(repo.git('rev-parse', r.branch)).toBe(repo.git('rev-parse', 'refs/remotes/origin/release/2'));
    });

    it('ohne Zielbranch bleibt der Start wie bisher: Abzweig vom HEAD, kein Hinweis', async () => {
      await ticket({ title: 'A' });
      const r = await manager.start('ARC-0001', {}, repo.path, f.actor);
      expect(r.targetCreated).toBeUndefined();
      expect(repo.git('rev-parse', r.branch)).toBe(repo.git('rev-parse', 'HEAD'));
    });

    it('tritt nur Worktrees mit gleichem Zielbranch bei', async () => {
      await ticket({ title: 'A', touches: ['a'], targetBranch: 'release/1' });
      await ticket({ title: 'B', touches: ['b'], targetBranch: 'release/2' });
      await manager.start('ARC-0001', {}, repo.path, f.actor);
      const viaIn = await manager.start('ARC-0002', { inTicket: 'ARC-0001' }, repo.path, f.actor);
      expect(viaIn).toMatchObject({ joined: false });
      expect(viaIn.warning).toMatch(/anderen Zielbranch/);
    });

    it('tritt mit join nur einem Worktree derselben Spec und desselben Zielbranches bei', async () => {
      await ticket({ title: 'A', touches: ['a'], spec: '001-x', specTask: 'T001', targetBranch: 'release/1' });
      await ticket({ title: 'B', touches: ['b'], spec: '001-x', specTask: 'T002', targetBranch: 'release/2' });
      await ticket({ title: 'C', touches: ['c'], spec: '001-x', specTask: 'T003', targetBranch: 'release/2' });
      await ticket({ title: 'D', touches: ['d'], spec: '001-x', specTask: 'T004' });
      const first = await manager.start('ARC-0001', { join: true }, repo.path, f.actor);
      const second = await manager.start('ARC-0002', { join: true }, repo.path, f.actor);
      expect(second).toMatchObject({ joined: false });
      expect(second.branch).not.toBe(first.branch);
      // gleiche Spec und gleicher Zielbranch: Beitritt zum zweiten, nicht zum ersten Worktree
      expect(await manager.joinCandidate('ARC-0003')).toBe(second.branch);
      expect(await manager.start('ARC-0003', { join: true }, repo.path, f.actor)).toMatchObject({ joined: true, branch: second.branch });
      // ohne Zielbranch tritt ein Ticket keinem Worktree mit Zielbranch bei
      expect(await manager.joinCandidate('ARC-0004')).toBeUndefined();
      expect(await manager.start('ARC-0004', { join: true }, repo.path, f.actor)).toMatchObject({ joined: false });
    });

    it('räumt nach Merge in einen Zielbranch außerhalb von HEAD auf', async () => {
      await ticket({ title: 'A', targetBranch: 'release/1' });
      const r = await manager.start('ARC-0001', {}, repo.path, f.actor);
      writeFileSync(join(r.path, 'a.txt'), 'a');
      execIn(r.path, 'add', 'a.txt');
      execIn(r.path, 'commit', '-q', '-m', 'a');
      const mergeWt = new TargetBranches().openMergeWorktree(repo.path, 'release/1');
      execIn(mergeWt, 'merge', '--no-ff', '-q', '--no-edit', r.branch);
      const out = await manager.removeMerged(r.branch, f.actor, 'release/1');
      expect(out.warnings).toEqual([]);
      expect(out.removed).toBe(r.path);
      expect(repo.git('branch', '--list', r.branch)).toBe('');
      expect(existsSync(r.path)).toBe(false);
      expect(await f.tickets.get('ARC-0001')).toMatchObject({ branch: null, worktree: null });
    });

    it('räumt nichts auf, solange der Branch nicht im Zielbranch steckt, auch wenn HEAD ihn schon enthält', async () => {
      await ticket({ title: 'A', targetBranch: 'release/1' });
      const r = await manager.start('ARC-0001', {}, repo.path, f.actor);
      writeFileSync(join(r.path, 'a.txt'), 'a');
      execIn(r.path, 'add', 'a.txt');
      execIn(r.path, 'commit', '-q', '-m', 'a');
      // Nur im Hauptordner gemergt, nicht in release/1
      execIn(repo.path, 'merge', '--no-ff', '-q', '--no-edit', r.branch);
      const out = await manager.removeMerged(r.branch, f.actor, 'release/1');
      expect(out).toMatchObject({ removed: null, tickets: ['ARC-0001'] });
      expect(out.warnings).toEqual([expect.stringMatching(/nicht gemergt/)]);
      expect(repo.git('branch', '--list', r.branch)).toContain(r.branch);
      expect(existsSync(r.path)).toBe(true);
    });
  });
});
