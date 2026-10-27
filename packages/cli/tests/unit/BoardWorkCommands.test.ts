import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { BoardServices, HarnessProject } from '@harness/core';
import { BoardWorkCommands } from '../../src/commands/board/BoardWorkCommands.js';
import { HarnessCommand } from '../../src/output/HarnessCommand.js';
import { Printer } from '../../src/output/Printer.js';
import type { CliContext } from '../../src/CliContext.js';
import { TempRepo } from '../../../core/tests/unit/helpers/TempRepo.js';
import { BoardFixture } from '../../../core/tests/unit/board/helpers/BoardFixture.js';

describe('harness board start (In-Memory-Board, echtes Git)', () => {
  let repo: TempRepo;
  let f: BoardFixture;
  let out: string[];
  let run: (...args: string[]) => Promise<unknown>;

  beforeEach(async () => {
    repo = new TempRepo();
    const project = await HarnessProject.init(repo.path);
    f = await BoardFixture.create();
    await f.boards.setRepoPath(f.boardId, project.root);
    const services = new BoardServices(f.db, f.clock, f.repoPaths);
    const rt = { services, config: { userId: f.actor.userId, activeBoard: null }, actor: (name?: string) => ({ userId: f.actor.userId, name: name ?? 'mensch' }) };
    const context = {
      withBoard: (work: (r: unknown) => Promise<unknown>) => work(rt),
      printer: () => new Printer(false),
      projectDir: () => repo.path,
      openProject: () => HarnessProject.open(repo.path),
      joinWorktrees: async () => false,
    } as unknown as CliContext;
    out = [];
    vi.spyOn(console, 'log').mockImplementation((line: string) => { out.push(line); });
    vi.spyOn(console, 'error').mockImplementation(() => undefined);
    run = (...args: string[]) => {
      const program = new HarnessCommand('harness').exitOverride();
      new BoardWorkCommands(context).register(program.command('board'));
      return program.parseAsync(['node', 'harness', 'board', ...args]);
    };
  });
  afterEach(() => {
    vi.restoreAllMocks();
    repo.dispose();
  });

  it('meldet einen neu angelegten Zielbranch als eigene Zeile', async () => {
    await f.tickets.create(f.boardId, { title: 'A', targetBranch: 'release/1' }, f.actor);
    await run('start', 'ARC-0001');
    expect(out.some((l) => /^Zielbranch release\/1 neu angelegt von \S+\.$/.test(l))).toBe(true);
    expect(out[0]).toContain('ARC-0001 läuft jetzt auf Branch ticket/ARC-0001-a');
  });

  it('gibt ohne Zielbranch keine Zielbranch-Zeile aus', async () => {
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    await run('start', 'ARC-0001');
    expect(out.some((l) => l.startsWith('Zielbranch'))).toBe(false);
  });
});
