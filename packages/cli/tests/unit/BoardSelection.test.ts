import { describe, expect, it } from 'vitest';
import { BoardServices, type BoardConfig } from '@harness/core';
import { BoardSelection } from '../../src/board/BoardSelection.js';
import { BoardFixture } from '../../../core/tests/unit/board/helpers/BoardFixture.js';

describe('BoardSelection', () => {
  // Laufzeit-Ersatz mit In-Memory-Diensten und aktivem Board
  const runtime = async () => {
    const f = await BoardFixture.create();
    const config = { activeBoard: f.boardId, userId: f.actor.userId } as BoardConfig;
    return { f, rt: { services: new BoardServices(f.db, f.clock, f.repoPaths), config } };
  };

  it('nimmt ein vorhandenes Board aus --board, sonst das aktive Board', async () => {
    const { f, rt } = await runtime();
    const selection = new BoardSelection();
    expect(await selection.one(rt, String(f.boardId))).toBe(f.boardId);
    expect(await selection.one(rt)).toBe(f.boardId);
    expect(await selection.many(rt, 'all')).toEqual([f.boardId]);
  });

  it('lehnt ein unbekanntes Board aus --board mit board-not-found ab', async () => {
    const { rt } = await runtime();
    const selection = new BoardSelection();
    await expect(selection.one(rt, '99')).rejects.toMatchObject({ code: 'board-not-found', message: 'Board 99 gibt es nicht.' });
    await expect(selection.many(rt, '99')).rejects.toMatchObject({ code: 'board-not-found' });
    await expect(selection.one(rt, 'x')).rejects.toMatchObject({ code: 'board-id-invalid' });
  });
});
