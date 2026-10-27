import { describe, expect, it } from 'vitest';
import type { Board } from '../../../src/board/BoardTypes.js';
import { RepoBoardResolver } from '../../../src/board/RepoBoardResolver.js';
import { BoardFixture } from './helpers/BoardFixture.js';

const ME = '11111111-1111-1111-1111-111111111111';
const OTHER = '22222222-2222-2222-2222-222222222222';
const board = (id: number, ownerId: string, repoPath: string | null): Board => ({ id, projectId: 1, ownerId, name: `B${id}`, repoPath, createdAt: '' });

describe('RepoBoardResolver', () => {
  const win = new RepoBoardResolver('win32');
  const linux = new RepoBoardResolver('linux');

  it('vereinheitlicht Schrägstriche, Endstrich und unter Windows die Schreibweise', () => {
    expect(win.normalize('C:\\Projects\\Arches\\')).toBe('c:/projects/arches');
    expect(linux.normalize('/home/x/Repo/')).toBe('/home/x/Repo');
  });

  it('nimmt das eigene aktive Board, sonst das eigene, sonst das erste', () => {
    const boards = [board(5, OTHER, 'C:/Repo'), board(3, ME, 'c:\\repo\\'), board(4, ME, 'C:/Repo'), board(9, ME, 'C:/Anderes')];
    expect(win.pick(boards, 'C:/Repo', { userId: ME, activeBoard: 4 })?.id).toBe(4);
    expect(win.pick(boards, 'C:/Repo', { userId: ME, activeBoard: 9 })?.id).toBe(3);
    expect(win.pick(boards, 'C:/Repo', { userId: '33333333-3333-3333-3333-333333333333', activeBoard: null })?.id).toBe(3);
    expect(win.pick(boards, 'D:/Fremd', { userId: ME, activeBoard: null })).toBeNull();
    expect(linux.pick([board(1, ME, '/home/Repo')], '/home/repo', { userId: ME, activeBoard: null })).toBeNull();
  });

  it('findet das Board über den Dienst und meldet ein fehlendes verständlich', async () => {
    const f = await BoardFixture.create();
    await f.boards.setRepoPath(f.boardId, 'C:\\Projects\\Demo');
    const me = { userId: f.actor.userId, activeBoard: null };
    expect((await f.boards.forRepo('C:/Projects/Demo', me))?.id).toBe(f.boardId);
    expect(await f.boards.forRepo('C:/Nirgends', me)).toBeNull();
    await expect(f.boards.requireForRepo('C:/Nirgends', me)).rejects.toMatchObject({
      code: 'board-repo-unknown', message: expect.stringContaining('harness board boards repo'),
    });
  });

  it('ordnet über den eigenen lokalen Pfad zu, nie über den DB-Wert eines fremden Boards', async () => {
    const f = await BoardFixture.create();
    const projectId = (await f.db.transaction((u) => u.projects.byKey('ARC')))!.id;
    const foreign = await f.db.transaction(async (u) => {
      await u.users.upsert({ id: OTHER, name: 'Kollege' });
      return u.boards.insert({ projectId, ownerId: OTHER, name: 'Fremd', repoPath: 'C:/Projects/Demo', createdAt: '' });
    });
    const me = { userId: f.actor.userId, activeBoard: null };
    expect(await f.boards.forRepo('C:/Projects/Demo', me)).toBeNull();
    await f.boards.setRepoPath(foreign.id, 'C:/Projects/Demo');
    expect((await f.boards.forRepo('C:/Projects/Demo', me))?.id).toBe(foreign.id);
    await f.boards.setRepoPath(f.boardId, 'C:/Projects/Demo');
    expect((await f.boards.forRepo('C:/Projects/Demo', me))?.id).toBe(f.boardId);
  });
});
