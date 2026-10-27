import { describe, it, expect, beforeEach } from 'vitest';
import { TicketAssigner } from '../../../src/hooks/TicketAssigner.js';
import { BoardServices } from '../../../src/board/BoardServices.js';
import { BoardFixture } from '../board/helpers/BoardFixture.js';

describe('TicketAssigner (DB-Board)', () => {
  let f: BoardFixture;
  let assigner: TicketAssigner;
  const input = (over: { cwd?: string; sessionId?: string; env?: Record<string, string>; boardId?: number | null } = {}) => ({
    cwd: over.cwd ?? '/w/repo', sessionId: over.sessionId ?? 's-1', env: over.env ?? {}, boardId: over.boardId === undefined ? f.boardId : over.boardId,
  });

  beforeEach(async () => {
    f = await BoardFixture.create();
    assigner = new TicketAssigner(new BoardServices(f.db, f.clock, f.repoPaths), 'linux');
    for (const title of ['A', 'B', 'C']) await f.tickets.create(f.boardId, { title }, f.actor);
    await f.tickets.update('ARC-0001', { status: 'in_progress', worktree: '/w/repo-worktrees/ARC-0001' }, f.actor);
    await f.tickets.update('ARC-0003', { status: 'in_progress', worktree: '/w/repo-worktrees/ARC-0001' }, f.actor);
  });

  it('nimmt HARNESS_TICKET vor allem anderen und übergeht unbekannte IDs', async () => {
    expect(await assigner.resolve(input({ env: { HARNESS_TICKET: 'arc-2, ARC-0099,kaputt' }, cwd: '/w/repo-worktrees/ARC-0001' }))).toEqual(['ARC-0002']);
  });

  it('nimmt Tickets, mit denen die Session verknüpft ist', async () => {
    await f.tickets.linkSession('ARC-0002', 's-9');
    expect(await assigner.resolve(input({ sessionId: 's-9', cwd: '/w/repo-worktrees/ARC-0001' }))).toEqual(['ARC-0002']);
  });

  it('ordnet über den Worktree zu (auch aus Unterordnern), alle laufenden, keine erledigten', async () => {
    expect(await assigner.resolve(input({ cwd: '/w/repo-worktrees/ARC-0001/src/x' }))).toEqual(['ARC-0001', 'ARC-0003']);
    await f.tickets.move('ARC-0003', 'done', f.actor, { acceptance: 'ok' });
    expect(await assigner.resolve(input({ cwd: '\\w\\repo-worktrees\\ARC-0001\\' }))).toEqual(['ARC-0001']);
  });

  it('verwechselt ähnlich benannte Ordner nicht und ordnet ohne Board oder im Hauptordner nichts zu', async () => {
    expect(await assigner.resolve(input({ cwd: '/w/repo-worktrees/ARC-00011' }))).toEqual([]);
    expect(await assigner.resolve(input({ cwd: '/w/repo' }))).toEqual([]);
    expect(await assigner.resolve(input({ cwd: '/w/repo-worktrees/ARC-0001', boardId: null }))).toEqual([]);
  });
});
