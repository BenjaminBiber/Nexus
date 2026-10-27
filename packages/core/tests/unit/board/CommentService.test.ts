import { describe, expect, it } from 'vitest';
import { BoardFixture } from './helpers/BoardFixture.js';

describe('CommentService', () => {
  it('hängt Kommentare an, erhöht die Version, schreibt keinen Verlauf', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    const c = await f.comments.add('arc-1', '  Schritt 2 fertig ', f.actor);
    expect(c).toMatchObject({ ticketId: 'ARC-0001', body: 'Schritt 2 fertig', authorName: 'claude', authorId: f.actor.userId });
    const detail = await f.tickets.get('ARC-0001');
    expect(detail.rev).toBe(2);
    expect(detail.history).toHaveLength(1);
    expect((await f.comments.list('ARC-0001')).map((x) => x.body)).toEqual(['Schritt 2 fertig']);
  });

  it('lehnt leere Kommentare und unbekannte Tickets ab', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    await expect(f.comments.add('ARC-0001', ' ', f.actor)).rejects.toMatchObject({ code: 'comment-empty' });
    await expect(f.comments.add('ARC-0009', 'x', f.actor)).rejects.toMatchObject({ code: 'ticket-not-found' });
  });
});
