import { afterEach, describe, expect, it, vi } from 'vitest';
import { BoardServices } from '@harness/core';
import { BoardBoardsCommands } from '../../src/commands/board/BoardBoardsCommands.js';
import { HarnessCommand } from '../../src/output/HarnessCommand.js';
import type { CliContext } from '../../src/CliContext.js';
import { BoardFixture } from '../../../core/tests/unit/board/helpers/BoardFixture.js';
import { Printer } from '../../src/output/Printer.js';

describe('harness board boards repo/list (In-Memory-Board, lokale Repo-Pfade)', () => {
  afterEach(() => vi.restoreAllMocks());

  const setup = async () => {
    const f = await BoardFixture.create();
    const services = new BoardServices(f.db, f.clock, f.repoPaths);
    const rt = { services, config: { activeBoard: f.boardId, userId: f.actor.userId }, actor: (name?: string) => ({ userId: f.actor.userId, name: name ?? 'mensch' }) };
    const context = {
      withBoard: (work: (r: unknown) => Promise<unknown>) => work(rt),
      printer: () => new Printer(false),
      projectDir: () => process.cwd(),
    } as unknown as CliContext;
    const out: string[] = [];
    vi.spyOn(console, 'log').mockImplementation((line: unknown) => { out.push(String(line)); });
    const run = (...args: string[]) => {
      const program = new HarnessCommand('harness').exitOverride();
      new BoardBoardsCommands(context).register(program.command('board'));
      return program.parseAsync(['node', 'harness', 'board', 'boards', ...args]);
    };
    return { f, run, out };
  };

  it('repo schreibt die lokale Zuordnung, nicht die DB', async () => {
    const { f, run, out } = await setup();
    await run('repo', String(f.boardId), 'C:/Projects/Arches');
    expect(await f.repoStore.read()).toEqual(new Map([[f.boardId, 'C:/Projects/Arches']]));
    expect((await f.db.transaction((u) => u.boards.byId(f.boardId)))?.repoPath).toBeNull();
    expect(out.join('\n')).toContain(`Mein Repo-Pfad von Board ${f.boardId}: C:/Projects/Arches`);
  });

  it('repo ohne Pfad zeigt den eigenen Pfad, --clear entfernt ihn', async () => {
    const { f, run, out } = await setup();
    await run('repo', String(f.boardId), 'C:/Projects/Arches');
    await run('repo', String(f.boardId));
    expect(await f.repoStore.read()).toEqual(new Map([[f.boardId, 'C:/Projects/Arches']]));
    expect(out.at(-1)).toContain('C:/Projects/Arches');
    await run('repo', String(f.boardId), '--clear');
    expect((await f.boards.require(f.boardId)).repoPath).toBeNull();
    expect(out.at(-1)).toContain('(keiner)');
  });

  it('repo ohne Pfad lässt die Zuordnung unverändert', async () => {
    const { f, run } = await setup();
    await run('repo', String(f.boardId), 'C:/Projects/Arches');
    const before = await f.repoStore.read();
    await run('repo', String(f.boardId));
    expect(await f.repoStore.read()).toEqual(before);
  });

  it('list zeigt den eigenen Pfad und kennzeichnet den DB-Altbestand', async () => {
    const { f, run, out } = await setup();
    await f.db.transaction(async (u) => {
      const own = (await u.boards.byId(f.boardId))!;
      await u.boards.update({ ...own, repoPath: 'C:/alt' });
    });
    await run('list');
    expect(out.join('\n')).toContain('C:/alt (aus der DB, Altbestand)');
    await run('repo', String(f.boardId), 'D:/neu');
    out.length = 0;
    await run('list');
    expect(out.join('\n')).toContain('D:/neu');
    expect(out.join('\n')).not.toContain('Altbestand');
  });
});
