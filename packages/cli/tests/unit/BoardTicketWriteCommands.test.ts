import { afterEach, describe, expect, it, vi } from 'vitest';
import { BoardServices } from '@harness/core';
import { BoardTicketWriteCommands } from '../../src/commands/board/BoardTicketWriteCommands.js';
import { HarnessCommand } from '../../src/output/HarnessCommand.js';
import type { CliContext } from '../../src/CliContext.js';
import { BoardFixture } from '../../../core/tests/unit/board/helpers/BoardFixture.js';
import { Printer } from '../../src/output/Printer.js';

describe('harness board set (Befehlsbaum mit In-Memory-Board)', () => {
  afterEach(() => vi.restoreAllMocks());

  // Programm mit `board set`; der Kontext liefert In-Memory-Dienste, Handelnder = --by
  const setup = async () => {
    const f = await BoardFixture.create();
    const services = new BoardServices(f.db, f.clock, f.repoPaths);
    const rt = { services, config: { activeBoard: f.boardId }, actor: (name?: string) => ({ userId: f.actor.userId, name: name ?? 'mensch' }) };
    const context = {
      withBoard: (work: (r: unknown) => Promise<unknown>) => work(rt),
      printer: () => new Printer(false),
      projectDir: () => process.cwd(),
    } as unknown as CliContext;
    vi.spyOn(console, 'log').mockImplementation(() => undefined);
    const run = (...args: string[]) => {
      const program = new HarnessCommand('harness').exitOverride();
      new BoardTicketWriteCommands(context).register(program.command('board'));
      return program.parseAsync(['node', 'harness', 'board', ...args]);
    };
    return { f, run };
  };

  it('create --target-branch legt das Ticket mit Zielbranch an; ohne Angabe bleibt er leer', async () => {
    const { f, run } = await setup();
    await run('create', 'A', '--board', String(f.boardId), '--target-branch', 'release/1');
    await run('create', 'B', '--board', String(f.boardId));
    expect(await f.tickets.get('ARC-0001')).toMatchObject({ targetBranch: 'release/1' });
    expect(await f.tickets.get('ARC-0002')).toMatchObject({ targetBranch: null });
  });

  it('set --target-branch setzt den Zielbranch, „none“ leert ihn', async () => {
    const { f, run } = await setup();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    await run('set', 'ARC-0001', '--target-branch', 'release/2');
    expect(await f.tickets.get('ARC-0001')).toMatchObject({ targetBranch: 'release/2' });
    await run('set', 'ARC-0001', '--target-branch', 'none');
    expect(await f.tickets.get('ARC-0001')).toMatchObject({ targetBranch: null });
  });

  it('set --target-branch meldet einen ungültigen Namen', async () => {
    const { f, run } = await setup();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    await expect(run('set', 'ARC-0001', '--target-branch', 'ticket/x')).rejects.toMatchObject({ code: 'target-branch-invalid' });
  });

  it('--clear-branch leert Branch und Worktree mit Verlaufseintrag', async () => {
    const { f, run } = await setup();
    await f.tickets.create(f.boardId, { title: 'Verirrt' }, f.actor);
    await f.tickets.update('ARC-0001', { branch: 'ticket/ARC-0002-fremd', worktree: 'C:/w/ARC-0002' }, f.actor);
    await run('set', 'ARC-0001', '--clear-branch', '--by', 'Florian');
    const detail = await f.tickets.get('ARC-0001');
    expect(detail).toMatchObject({ branch: null, worktree: null });
    expect(detail.history.filter((h) => h.byName === 'Florian').map((h) => [h.field, h.from, h.to])).toEqual([
      ['branch', 'ticket/ARC-0002-fremd', null],
      ['worktree', 'C:/w/ARC-0002', null],
    ]);
  });
});
