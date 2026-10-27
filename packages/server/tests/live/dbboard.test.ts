import { describe, expect, it } from 'vitest';
import { LiveServer } from '../helpers/LiveServer.js';
import { MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';

describe('Live-Meldungen der Board-Datenbank', () => {
  it('meldet eine Änderung am beobachteten Board als dbboard', async () => {
    const backend = await MemoryBoardBackend.create();
    const live = await LiveServer.start(15000, { boardBackend: async () => backend, dbPollMs: 50 });
    try {
      const { id } = await live.server.addProject();
      await backend.services.projects.create('ARC', 'Arches', backend.actor());
      const board = await backend.services.boards.create('ARC', 'Sprint', backend.actor());
      const stream = await live.listen(id, `?dbBoard=${board.id}`);
      expect(await LiveServer.until(() => stream.events.some((e) => e.type === 'hello'))).toBe(true);
      await new Promise((r) => setTimeout(r, 150));
      await backend.services.tickets.create(board.id, { title: 'Neu' }, backend.actor());
      expect(await LiveServer.until(() => stream.events.some((e) => e.type === 'dbboard' && e.boardId === board.id))).toBe(true);
      await stream.stop();
    } finally {
      await live.server.dispose();
    }
  });
});
