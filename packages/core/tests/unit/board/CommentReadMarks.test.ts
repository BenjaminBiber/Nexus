import { describe, expect, it } from 'vitest';
import { BoardFixture } from './helpers/BoardFixture.js';

describe('Lesemarke der Kommentare', () => {
  it('liefert nur Kommentare ohne Lesemarke; Kommentare aus einer Session gelten als gelesen', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    await f.tickets.create(f.boardId, { title: 'B' }, f.actor);
    await f.comments.add('ARC-0001', 'Bitte prüfen', { userId: f.actor.userId, name: 'Florian' });
    const own = await f.comments.add('ARC-0001', 'Zwischenstand', f.actor, { readByAgent: true });
    await f.comments.add('ARC-0002', 'Auch hier', { userId: f.actor.userId, name: 'Florian' });
    expect(own.readByAgentAt).toBe(own.createdAt);
    const unread = await f.comments.unreadForAgent(['ARC-0001', 'ARC-0002']);
    expect(unread.map((c) => [c.ticketId, c.body])).toEqual([['ARC-0001', 'Bitte prüfen'], ['ARC-0002', 'Auch hier']]);
    f.advance(5);
    await f.comments.markReadByAgent(unread.map((c) => c.id));
    expect(await f.comments.unreadForAgent(['ARC-0001', 'ARC-0002'])).toEqual([]);
    expect((await f.comments.list('ARC-0001'))[0].readByAgentAt).toBe('2026-10-13T10:05:00.000Z');
  });

  it('kommt mit leeren Listen aus', async () => {
    const f = await BoardFixture.create();
    expect(await f.comments.unreadForAgent([])).toEqual([]);
    await expect(f.comments.markReadByAgent([])).resolves.toBeUndefined();
  });
});
