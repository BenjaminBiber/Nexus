import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { BoardServices, HarnessProject } from '@harness/core';
import { HookRunner } from '../../src/hooks/HookRunner.js';
import type { BoardAccess } from '../../src/hooks/BoardAccess.js';
import { TempRepo } from '../../../core/tests/unit/helpers/TempRepo.js';
import { BoardFixture } from '../../../core/tests/unit/board/helpers/BoardFixture.js';

const HUMAN = { userId: '11111111-1111-1111-1111-111111111111', name: 'Florian' };
// HookRunner liest Home und Port aus der übergebenen Umgebung, nicht aus process.env: immer die Test-Umgebung mitgeben
const sandbox = () => ({ HARNESS_HOME: process.env.HARNESS_HOME!, HARNESS_PORT: process.env.HARNESS_PORT! });

describe('HookRunner (DB-Board)', () => {
  let repo: TempRepo;
  let f: BoardFixture;
  let access: BoardAccess;
  let worktree: string;
  let outputs: string[];

  // Führt einen Hook aus und liefert die letzte Ausgabe als Objekt (oder null)
  const hook = async (kind: 'session-start' | 'user-prompt' | 'stop', payload: object, env: Record<string, string> = {}, board: BoardAccess | null = access) => {
    outputs = [];
    const runner = new HookRunner({ openBoard: async () => board, launcher: { probe: async () => 'foreign', start: async () => undefined } });
    await runner.run(kind, payload, { ...sandbox(), HARNESS_DEBUG: '1', ...env }, (o) => outputs.push(o));
    return outputs.length ? (JSON.parse(outputs.at(-1)!) as { hookSpecificOutput: Record<string, string> }).hookSpecificOutput : null;
  };

  beforeEach(async () => {
    repo = new TempRepo();
    const project = await HarnessProject.init(repo.path);
    f = await BoardFixture.create();
    await f.boards.setRepoPath(f.boardId, project.root);
    access = { services: new BoardServices(f.db, f.clock, f.repoPaths), config: { userId: HUMAN.userId, activeBoard: null }, actor: (n) => ({ userId: HUMAN.userId, name: n ?? 'Test' }), close: async () => undefined };
    // Echter Git-Worktree: HarnessProject.open braucht ein Git-Verzeichnis
    worktree = repo.addWorktree('wt-ARC-0001').replace(/\\/g, '/');
    await f.tickets.create(f.boardId, { title: 'Login', spec: '001-x' }, f.actor);
    await f.tickets.update('ARC-0001', { status: 'in_progress', worktree }, f.actor);
  });
  afterEach(() => repo.dispose());

  it('session-start im Worktree: Kontext mit Ticket, Umgebungsvariablen mit Session und Ticket', async () => {
    const envFile = join(repo.base, 'claude-env.sh');
    const out = await hook('session-start', { session_id: 's1', cwd: worktree }, { CLAUDE_ENV_FILE: envFile });
    expect(out?.hookEventName).toBe('SessionStart');
    expect(out?.additionalContext).toContain('ARC-0001 „Login“ (Spec 001-x)');
    expect(readFileSync(envFile, 'utf8')).toBe("export HARNESS_SESSION='s1'\nexport HARNESS_TICKET='ARC-0001'\n");
  });

  it('user-prompt speist einen Kommentar genau einmal ein', async () => {
    await f.comments.add('ARC-0001', 'Bitte Tastatur beachten', HUMAN);
    const first = await hook('user-prompt', { session_id: 's1', cwd: worktree });
    expect(first?.additionalContext).toContain('Bitte Tastatur beachten');
    const second = await hook('user-prompt', { session_id: 's1', cwd: worktree });
    expect(second?.additionalContext).toBeUndefined();
  });

  it('beachtet HARNESS_TICKET und verknüpfte Sessions außerhalb des Worktrees', async () => {
    await f.tickets.create(f.boardId, { title: 'Zweites' }, f.actor);
    expect((await hook('session-start', { session_id: 's2', cwd: repo.path }, { HARNESS_TICKET: 'ARC-0002' }))?.additionalContext).toContain('ARC-0002 „Zweites“');
    await f.tickets.linkSession('ARC-0002', 's3');
    expect((await hook('session-start', { session_id: 's3', cwd: repo.path }))?.additionalContext).toContain('ARC-0002');
  });

  it('weist ohne Ticket auf claim hin', async () => {
    expect((await hook('session-start', { session_id: 's4', cwd: repo.path }))?.additionalContext).toContain('harness board claim');
  });

  it('bleibt ohne Board des Repos und ohne erreichbare DB still (nur der Session-Name aus dem Prompt geht zurück)', async () => {
    await f.boards.setRepoPath(f.boardId, null);
    expect(await hook('session-start', { session_id: 's5', cwd: worktree })).toBeNull();
    const noDb = await hook('user-prompt', { session_id: 's5', cwd: worktree, prompt: 'Login fertig machen' }, {}, null);
    expect(noDb?.additionalContext).toBeUndefined();
    expect(noDb?.sessionTitle).toMatch(/^SE-\d+: /);
  });

  it('wartet nicht auf eine hängende DB: Umgebung, Session-Name und Meldung kommen vor der Frist, ohne Kontext', async () => {
    const envFile = join(repo.base, 'claude-env.sh');
    const probes: number[] = [];
    const launcher = { probe: async (port: number) => (probes.push(port), 'foreign' as const), start: async () => undefined };
    const hanging = new Promise<BoardAccess | null>(() => undefined);
    const run = async (kind: 'session-start' | 'user-prompt', payload: object, env: Record<string, string> = {}) => {
      outputs = [];
      const runner = new HookRunner({ openBoard: () => hanging, launcher, boardBudgetMs: 300 });
      const started = Date.now();
      await runner.run(kind, payload, { ...sandbox(), ...env }, (o) => outputs.push(o));
      return Date.now() - started;
    };
    expect(await run('session-start', { session_id: 's6', cwd: worktree }, { CLAUDE_ENV_FILE: envFile })).toBeLessThan(2000);
    expect(readFileSync(envFile, 'utf8')).toBe("export HARNESS_SESSION='s6'\n");
    expect(outputs).toEqual([]);
    expect(await run('user-prompt', { session_id: 's6', cwd: worktree, prompt: 'Login fertig machen' })).toBeLessThan(2000);
    const out = JSON.parse(outputs[0]).hookSpecificOutput;
    expect(out.additionalContext).toBeUndefined();
    expect(out.sessionTitle).toMatch(/^SE-\d+: /);
    expect(probes).toHaveLength(2);
  });

  it('wartet auch nicht auf hängende Abfragen einer geöffneten DB', async () => {
    const boards = { ...access.services.boards, forRepo: () => new Promise(() => undefined) };
    const slow = { ...access, services: { ...access.services, boards } } as unknown as BoardAccess;
    const runner = new HookRunner({ openBoard: async () => slow, launcher: { probe: async () => 'foreign', start: async () => undefined }, boardBudgetMs: 300 });
    const envFile = join(repo.base, 'claude-env.sh');
    const started = Date.now();
    await runner.run('session-start', { session_id: 's7', cwd: worktree }, { ...sandbox(), CLAUDE_ENV_FILE: envFile }, () => undefined);
    expect(Date.now() - started).toBeLessThan(2000);
    expect(readFileSync(envFile, 'utf8')).toBe("export HARNESS_SESSION='s7'\n");
  });

  it('markiert Kommentare erst nach der Ausgabe als gelesen; scheitert die Ausgabe, kommen sie beim nächsten Mal wieder', async () => {
    await f.comments.add('ARC-0001', 'Bitte Tastatur beachten', HUMAN);
    const runner = new HookRunner({ openBoard: async () => access, launcher: { probe: async () => 'foreign', start: async () => undefined } });
    await runner.run('user-prompt', { session_id: 's1', cwd: worktree }, sandbox(), () => {
      throw new Error('Ausgabe geschlossen');
    });
    expect(await f.comments.unreadForAgent(['ARC-0001'])).toHaveLength(1);
    expect((await hook('user-prompt', { session_id: 's1', cwd: worktree }))?.additionalContext).toContain('Bitte Tastatur beachten');
    expect(await f.comments.unreadForAgent(['ARC-0001'])).toHaveLength(0);
  });

  it('behält bei einem DB-Fehler im Kontext die Zuordnung, speist aber keinen Kontext und keinen claim-Hinweis ein', async () => {
    access.services.tickets.get = () => Promise.reject(new Error('Verbindung unterbrochen'));
    const envFile = join(repo.base, 'claude-env.sh');
    const out = await hook('session-start', { session_id: 's8', cwd: worktree }, { CLAUDE_ENV_FILE: envFile });
    expect(out?.additionalContext).toBeUndefined();
    expect(readFileSync(envFile, 'utf8')).toBe("export HARNESS_SESSION='s8'\nexport HARNESS_TICKET='ARC-0001'\n");
  });

  it('stop legt keine Tickets mehr aus einer tasks.md an', async () => {
    mkdirSync(join(repo.path, 'specs', '001-x'), { recursive: true });
    writeFileSync(join(repo.path, 'specs', '001-x', 'tasks.md'), '- [ ] T001 Neue Aufgabe\n');
    const before = (await f.tickets.list({ boardIds: [f.boardId] })).length;
    await hook('stop', { session_id: 's1', cwd: repo.path });
    expect(await f.tickets.list({ boardIds: [f.boardId] })).toHaveLength(before);
  });
});
