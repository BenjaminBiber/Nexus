import { existsSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TempRepo } from '../helpers/TempRepo.js';
import { HarnessProject } from '../../../src/project/HarnessProject.js';
import { WorktreeManager } from '../../../src/worktrees/WorktreeManager.js';
import { BranchMerger } from '../../../src/dispatch/git/BranchMerger.js';
import { BoardServices } from '../../../src/board/BoardServices.js';
import { BoardFixture } from '../board/helpers/BoardFixture.js';
import type { NewTicketInput } from '../../../src/board/TicketService.js';

describe('WorktreeManager für den Dispatcher', () => {
  let repo: TempRepo;
  let f: BoardFixture;
  let manager: WorktreeManager;
  const ticket = (input: NewTicketInput) => f.tickets.create(f.boardId, input, f.actor);

  beforeEach(async () => {
    repo = new TempRepo();
    repo.git('config', 'user.name', 't');
    repo.git('config', 'user.email', 't@t');
    const project = await HarnessProject.init(repo.path);
    f = await BoardFixture.create();
    manager = new WorktreeManager(project.layout, new BoardServices(f.db, f.clock, f.repoPaths), project.root);
  });
  afterEach(() => repo.dispose());

  it('tritt mit joinOnlySameSpec ohne Spec nie automatisch bei', async () => {
    await ticket({ title: 'Erstes', touches: ['a'] });
    await ticket({ title: 'Zweites', touches: ['b'] });
    const first = await manager.start('ARC-0001', {}, repo.path, f.actor);
    const second = await manager.start('ARC-0002', { join: true, joinOnlySameSpec: true }, repo.path, f.actor);
    expect(second.joined).toBe(false);
    expect(second.branch).not.toBe(first.branch);
  });

  it('tritt mit joinOnlySameSpec derselben Spec weiter bei', async () => {
    await ticket({ title: 'Erstes', touches: ['a'], spec: '001-x', specTask: 'T001' });
    await ticket({ title: 'Zweites', touches: ['b'], spec: '001-x', specTask: 'T002' });
    const first = await manager.start('ARC-0001', {}, repo.path, f.actor);
    expect((await manager.start('ARC-0002', { join: true, joinOnlySameSpec: true }, repo.path, f.actor)).branch).toBe(first.branch);
    expect(await manager.ticketsOf(first.branch)).toEqual(['ARC-0001', 'ARC-0002']);
  });

  it('joinCandidate nennt den Branch, dem start beitreten würde, ohne etwas zu ändern', async () => {
    await ticket({ title: 'Erstes', touches: ['a'], spec: '001-x', specTask: 'T001' });
    await ticket({ title: 'Zweites', touches: ['b'], spec: '001-x', specTask: 'T002' });
    await ticket({ title: 'Ohne Spec', touches: ['c'] });
    expect(await manager.joinCandidate('ARC-0002', { joinOnlySameSpec: true })).toBeUndefined();
    const first = await manager.start('ARC-0001', {}, repo.path, f.actor);
    expect(await manager.joinCandidate('ARC-0002', { joinOnlySameSpec: true })).toBe(first.branch);
    expect(await manager.joinCandidate('ARC-0003', { joinOnlySameSpec: true })).toBeUndefined();
    expect(await manager.joinCandidate('ARC-0001', { joinOnlySameSpec: true })).toBeUndefined();
    expect(await manager.ticketsOf(first.branch)).toEqual(['ARC-0001']);
    expect((await f.tickets.get('ARC-0002')).branch).toBeNull();
  });

  it('legt einen fehlenden Worktree aus dem Branch neu an', async () => {
    await ticket({ title: 'Erstes' });
    const started = await manager.start('ARC-0001', {}, repo.path, f.actor);
    repo.git('worktree', 'remove', '--force', started.path);
    await manager.registry.markRemoved(started.branch);
    expect(existsSync(started.path)).toBe(false);
    expect(await manager.ensureWorktree('ARC-0001')).toBe(started.path);
    expect(existsSync(started.path)).toBe(true);
    expect((await manager.registry.read())[started.branch].removed).toBe(false);
    expect(await manager.ensureWorktree('ARC-0001')).toBe(started.path);
  });

  it('resume: eigener Worktree wird genutzt, ein fehlender Ordner neu angelegt', async () => {
    await ticket({ title: 'Erstes' });
    const started = await manager.start('ARC-0001', {}, repo.path, f.actor);
    expect(await manager.resume('ARC-0001', f.actor)).toEqual({ path: started.path });
    repo.git('worktree', 'remove', '--force', started.path);
    expect(await manager.resume('ARC-0001', f.actor)).toEqual({ path: started.path });
    expect(existsSync(started.path)).toBe(true);
  });

  it('resume: Worktree eines anderen Tickets wird nicht wiederhergestellt, Branch und Worktree am Ticket geleert', async () => {
    await ticket({ title: 'Erstes' });
    await ticket({ title: 'Zweites' });
    const first = await manager.start('ARC-0001', {}, repo.path, f.actor);
    repo.git('worktree', 'remove', '--force', first.path);
    await f.tickets.update('ARC-0002', { branch: first.branch, worktree: first.path }, f.actor);
    const result = await manager.resume('ARC-0002', f.actor);
    expect(result).toMatchObject({ reset: expect.stringContaining('gehört ARC-0001') });
    expect(existsSync(first.path)).toBe(false);
    expect(await f.tickets.get('ARC-0002')).toMatchObject({ branch: null, worktree: null });
    expect(await manager.ticketsOf(first.branch)).toEqual(['ARC-0001']);
  });

  it('resume: ein aufgeräumter Worktree wird nicht wiederhergestellt; der alte Branch bleibt', async () => {
    await ticket({ title: 'Erstes' });
    const started = await manager.start('ARC-0001', {}, repo.path, f.actor);
    repo.git('worktree', 'remove', '--force', started.path);
    await manager.registry.markRemoved(started.branch);
    expect(await manager.resume('ARC-0001', f.actor)).toMatchObject({ reset: expect.stringContaining('wurde aufgeräumt') });
    expect(existsSync(started.path)).toBe(false);
    expect(await f.tickets.get('ARC-0001')).toMatchObject({ branch: null, worktree: null });
    expect(repo.git('branch', '--list', started.branch)).not.toBe('');
  });

  it('ensureWorktree lehnt ungestartete Tickets ab', async () => {
    await ticket({ title: 'Erstes' });
    await expect(manager.ensureWorktree('ARC-0001')).rejects.toMatchObject({ code: 'ticket-not-started' });
  });

  it('räumt nach dem Merge Worktree und Branch auf und leert die Ticket-Felder', async () => {
    await ticket({ title: 'Erstes', acceptance: 'ok' });
    const started = await manager.start('ARC-0001', {}, repo.path, f.actor);
    writeFileSync(join(started.path, 'neu.txt'), 'x');
    repo.git('-C', started.path, 'add', 'neu.txt');
    repo.git('-C', started.path, 'commit', '-q', '-m', 'neu');
    await manager.finish('ARC-0001', {}, started.path, f.actor);
    expect(await manager.isReady(started.branch)).toBe(true);
    repo.git('merge', '--no-ff', '--no-edit', started.branch);
    const result = await manager.removeMerged(started.branch, f.actor);
    expect(result).toEqual({ removed: started.path, tickets: ['ARC-0001'], warnings: [] });
    expect(existsSync(started.path)).toBe(false);
    expect(repo.git('branch', '--list', started.branch)).toBe('');
    expect(await f.tickets.get('ARC-0001')).toMatchObject({ status: 'done', branch: null, worktree: null });
  });

  // Hauptordner mit echtem node_modules (ignoriert) und Worktree, in dem ein Agent eine Junction darauf angelegt hat
  async function withJunction(): Promise<{ started: { branch: string; path: string }; modules: string }> {
    writeFileSync(join(repo.path, '.gitignore'), 'node_modules/\nnode_modules\n');
    repo.git('add', '.gitignore');
    repo.git('commit', '-q', '-m', 'ignore');
    const modules = join(repo.path, 'node_modules');
    mkdirSync(join(modules, '.bin'), { recursive: true });
    writeFileSync(join(modules, '.package-lock.json'), '{}');
    writeFileSync(join(modules, '.bin', 'tool'), 'x');
    await ticket({ title: 'Erstes', acceptance: 'ok' });
    const started = await manager.start('ARC-0001', {}, repo.path, f.actor);
    symlinkSync(modules, join(started.path, 'node_modules'), 'junction');
    return { started, modules };
  }

  it('removeMerged entfernt eine Junction im Worktree nur als Link: node_modules des Hauptordners bleiben', async () => {
    const { started, modules } = await withJunction();
    writeFileSync(join(started.path, 'neu.txt'), 'x');
    repo.git('-C', started.path, 'add', 'neu.txt');
    repo.git('-C', started.path, 'commit', '-q', '-m', 'neu');
    await manager.finish('ARC-0001', {}, started.path, f.actor);
    repo.git('merge', '--no-ff', '--no-edit', started.branch);
    expect(await manager.removeMerged(started.branch, f.actor)).toMatchObject({ removed: started.path, warnings: [] });
    expect(existsSync(started.path)).toBe(false);
    expect(readFileSync(join(modules, '.package-lock.json'), 'utf8')).toBe('{}');
    expect(readFileSync(join(modules, '.bin', 'tool'), 'utf8')).toBe('x');
  });

  it('done räumt einen fertigen Worktree mit Junction auf, ohne das Ziel anzufassen', async () => {
    const { started, modules } = await withJunction();
    const done = await manager.finish('ARC-0001', {}, repo.path, f.actor);
    expect(done.removed).toEqual([started.path]);
    expect(existsSync(started.path)).toBe(false);
    expect(readFileSync(join(modules, '.package-lock.json'), 'utf8')).toBe('{}');
    expect(readFileSync(join(modules, '.bin', 'tool'), 'utf8')).toBe('x');
  });

  it('lässt einen Worktree mit ungesicherten Änderungen stehen und warnt', async () => {
    await ticket({ title: 'Erstes', acceptance: 'ok' });
    const started = await manager.start('ARC-0001', {}, repo.path, f.actor);
    writeFileSync(join(started.path, 'offen.txt'), 'x');
    const result = await manager.removeMerged(started.branch, f.actor);
    expect(result.removed).toBeNull();
    expect(result.warnings[0]).toContain('ungesicherte Änderungen');
    expect(existsSync(started.path)).toBe(true);
    rmSync(join(started.path, 'offen.txt'));
  });

  // Startet ARC-0001 und committet eine Datei im Worktree; mit merge auch in den Hauptordner gemergt
  const startWithCommit = async (merge: boolean) => {
    await ticket({ title: 'Erstes', acceptance: 'ok' });
    const started = await manager.start('ARC-0001', {}, repo.path, f.actor);
    writeFileSync(join(started.path, 'neu.txt'), 'x');
    repo.git('-C', started.path, 'add', 'neu.txt');
    repo.git('-C', started.path, 'commit', '-q', '-m', 'neu');
    if (merge) repo.git('merge', '--no-ff', '--no-edit', started.branch);
    return started;
  };

  it('fasst einen Ordner nicht an, der kein Worktree des gemergten Branches ist', async () => {
    const started = await startWithCommit(true);
    // Der eingetragene Ordner gehört inzwischen zu einem anderen Branch (samt Plugin-Einstellung)
    repo.git('worktree', 'remove', '--force', started.path);
    repo.git('worktree', 'add', '-q', '-b', 'anderer', started.path);
    mkdirSync(join(started.path, '.claude'), { recursive: true });
    writeFileSync(join(started.path, '.claude', 'settings.local.json'), '{}');
    const result = await manager.removeMerged(started.branch, f.actor);
    expect(result.removed).toBeNull();
    expect(result.warnings).toEqual([`Der Ordner ${started.path} ist kein Worktree von Branch ${started.branch}; er wird nicht angefasst. Bitte von Hand prüfen.`]);
    expect(existsSync(join(started.path, '.claude', 'settings.local.json'))).toBe(true);
    expect(repo.git('branch', '--list', started.branch)).not.toBe('');
    expect(await f.tickets.get('ARC-0001')).toMatchObject({ branch: started.branch, worktree: started.path });
  });

  it('räumt einen leeren Rest-Ordner eines halb entfernten Worktrees weg und schließt das Aufräumen ab', async () => {
    const started = await startWithCommit(true);
    // Windows: `worktree remove` löscht Inhalt und Verwaltungseintrag, der gesperrte leere Ordner bleibt stehen
    repo.git('worktree', 'remove', '--force', started.path);
    mkdirSync(started.path);
    const result = await manager.removeMerged(started.branch, f.actor);
    expect(result).toEqual({ removed: started.path, tickets: ['ARC-0001'], warnings: [] });
    expect(existsSync(started.path)).toBe(false);
    expect((await manager.registry.read())[started.branch].removed).toBe(true);
    expect(repo.git('branch', '--list', started.branch)).toBe('');
    expect(await f.tickets.get('ARC-0001')).toMatchObject({ branch: null, worktree: null });
  });

  it('räumt nach einem Squash Worktree und Branch auf, obwohl der Branch kein Vorfahre ist', async () => {
    const started = await startWithCommit(false);
    await manager.finish('ARC-0001', {}, started.path, f.actor);
    expect(new BranchMerger().merge(repo.path, started.branch, 'ARC-0001 (squash)', 'squash').kind).toBe('merged');
    const result = await manager.removeMerged(started.branch, f.actor);
    expect(result).toEqual({ removed: started.path, tickets: ['ARC-0001'], warnings: [] });
    expect(existsSync(started.path)).toBe(false);
    expect(repo.git('branch', '--list', started.branch)).toBe('');
    expect(await f.tickets.get('ARC-0001')).toMatchObject({ branch: null, worktree: null });
  });

  it('räumt nichts auf, wenn der Branch nach dem Squash weitere Commits bekam', async () => {
    const started = await startWithCommit(false);
    new BranchMerger().merge(repo.path, started.branch, 'ARC-0001 (squash)', 'squash');
    writeFileSync(join(started.path, 'danach.txt'), 'x');
    repo.git('-C', started.path, 'add', 'danach.txt');
    repo.git('-C', started.path, 'commit', '-q', '-m', 'danach');
    const result = await manager.removeMerged(started.branch, f.actor);
    expect(result.warnings).toEqual([`Branch ${started.branch} ist nicht gemergt; Worktree und Branch bleiben.`]);
    expect(existsSync(started.path)).toBe(true);
    expect(repo.git('branch', '--list', started.branch)).not.toBe('');
  });

  it('räumt nichts auf, solange der Branch nicht im Hauptordner gemergt ist', async () => {
    const started = await startWithCommit(false);
    const result = await manager.removeMerged(started.branch, f.actor);
    expect(result).toEqual({ removed: null, tickets: ['ARC-0001'], warnings: [`Branch ${started.branch} ist nicht gemergt; Worktree und Branch bleiben.`] });
    expect(existsSync(started.path)).toBe(true);
    expect(repo.git('branch', '--list', started.branch)).not.toBe('');
    expect(await f.tickets.get('ARC-0001')).toMatchObject({ branch: started.branch, worktree: started.path });
  });

  it('behält die Ticket-Felder, wenn der Branch nicht gelöscht werden kann', async () => {
    const started = await startWithCommit(true);
    repo.git('worktree', 'add', '-q', '--force', join(repo.base, 'zweit'), started.branch);
    const result = await manager.removeMerged(started.branch, f.actor);
    expect(result.removed).toBe(started.path);
    expect(result.warnings).toHaveLength(1);
    expect(result.warnings[0]).toContain('nicht gelöscht');
    expect(await f.tickets.get('ARC-0001')).toMatchObject({ branch: started.branch, worktree: started.path });
  });

  it('entfernt einen gesperrten Worktree nicht mit Gewalt, sondern warnt', async () => {
    const started = await startWithCommit(true);
    repo.git('worktree', 'lock', started.path);
    const result = await manager.removeMerged(started.branch, f.actor);
    expect(result.removed).toBeNull();
    expect(result.warnings[0]).toContain('nicht entfernt');
    expect(existsSync(started.path)).toBe(true);
    expect(existsSync(join(started.path, '.claude', 'settings.local.json'))).toBe(true);
    expect(repo.git('branch', '--list', started.branch)).not.toBe('');
    expect(await f.tickets.get('ARC-0001')).toMatchObject({ branch: started.branch, worktree: started.path });
    repo.git('worktree', 'unlock', started.path);
  });

  it('entfernt nie den Hauptordner selbst', async () => {
    await ticket({ title: 'Erstes' });
    repo.git('branch', 'ticket/haupt');
    await manager.registry.add('ticket/haupt', process.platform === 'win32' ? repo.path.toUpperCase() : repo.path, 'ARC-0001');
    const result = await manager.removeMerged('ticket/haupt', f.actor);
    expect(result.removed).toBeNull();
    expect(result.warnings[0]).toContain('Hauptordner');
    expect(existsSync(join(repo.path, '.git'))).toBe(true);
    expect(repo.git('branch', '--list', 'ticket/haupt')).not.toBe('');
  });

  it('ensureWorktree lehnt einen vorhandenen Ordner ab, der kein Worktree des Branches ist', async () => {
    await ticket({ title: 'Erstes' });
    const started = await manager.start('ARC-0001', {}, repo.path, f.actor);
    repo.git('worktree', 'remove', '--force', started.path);
    mkdirSync(started.path, { recursive: true });
    await expect(manager.ensureWorktree('ARC-0001')).rejects.toMatchObject({ code: 'worktree-mismatch' });
    rmSync(started.path, { recursive: true, force: true });
    repo.git('worktree', 'add', '-q', '-b', 'anders', started.path);
    await expect(manager.ensureWorktree('ARC-0001')).rejects.toMatchObject({ code: 'worktree-mismatch' });
  });
});
