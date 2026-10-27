import { existsSync } from 'node:fs';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';

const keys = (json: { items: Array<{ key: string }> }) => json.items.map((i) => i.key);

describe('/api/db/inbox', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;
  let boardId: number;
  let second: number;

  beforeEach(async () => {
    ({ server, backend } = await TestServer.startWithBoard());
    const s = backend.services;
    await s.projects.create('ARC', 'Arches', backend.actor());
    boardId = (await s.boards.create('ARC', 'Sprint', backend.actor())).id;
    second = (await s.boards.create('ARC', 'Zweites', backend.actor())).id;
    await s.tickets.create(boardId, { title: 'Abnahme', status: 'blocked' }, backend.actor());
    await s.tickets.update('ARC-0001', { blockedReason: 'Human Interaction: ansehen' }, backend.actor());
    await s.tickets.create(boardId, { title: 'Merge', status: 'blocked' }, backend.actor());
    await s.tickets.update('ARC-0002', { blockedReason: 'Merge: Konflikt' }, backend.actor());
    await s.tickets.create(second, { title: 'Planen', status: 'in_planning' }, backend.actor());
  });
  afterEach(() => server.dispose());

  it('liefert die Einträge nach Dringlichkeit mit Zählern', async () => {
    const res = await server.get('/api/db/inbox');
    expect(res.status).toBe(200);
    expect(keys(res.json)).toEqual(['ticket:ARC-0002', 'ticket:ARC-0001', 'ticket:ARC-0003']);
    expect(res.json.total).toBe(3);
    expect(res.json.counts).toMatchObject({ merge: 1, accept: 1, planning: 1, note: 0 });
    expect(res.json.items[0]).toMatchObject({ kind: 'merge', boardName: 'Sprint', projectKey: 'ARC', ticketId: 'ARC-0002' });
  });

  it('filtert nach Board und lehnt ungültige Boards ab', async () => {
    expect(keys((await server.get(`/api/db/inbox?board=${second}`)).json)).toEqual(['ticket:ARC-0003']);
    expect((await server.get('/api/db/inbox?board=abc')).status).toBe(422);
    expect((await server.get('/api/db/inbox?board=999')).json.code).toBe('board-not-owned');
  });

  it('speichert die Reihenfolge, wendet sie an und setzt sie zurück', async () => {
    const put = await server.put('/api/db/inbox/order', { keys: ['ticket:ARC-0003', 'ticket:ARC-0001'] });
    expect(put.status).toBe(200);
    expect(existsSync(server.home.inboxOrderFile)).toBe(true);
    // ARC-0002 hat keine Position und steht als neu oben
    expect(keys((await server.get('/api/db/inbox')).json)).toEqual(['ticket:ARC-0002', 'ticket:ARC-0003', 'ticket:ARC-0001']);

    expect((await server.delete('/api/db/inbox/order')).status).toBe(200);
    expect(existsSync(server.home.inboxOrderFile)).toBe(false);
    expect(keys((await server.get('/api/db/inbox')).json)).toEqual(['ticket:ARC-0002', 'ticket:ARC-0001', 'ticket:ARC-0003']);
  });

  it('lehnt ungültige Schlüssel und zu lange Listen ab', async () => {
    expect((await server.put('/api/db/inbox/order', { keys: ['quatsch'] })).status).toBe(422);
    expect((await server.put('/api/db/inbox/order', { keys: [42] })).json.code).toBe('input-invalid');
    expect((await server.put('/api/db/inbox/order', { keys: `ticket:${'A'.repeat(60)}` })).status).toBe(422);
    expect((await server.put('/api/db/inbox/order', { keys: [`ticket:${'A'.repeat(60)}`] })).status).toBe(422);
    expect((await server.put('/api/db/inbox/order', { keys: Array.from({ length: 1001 }, (_, i) => `note:${i}`) })).status).toBe(422);
    expect((await server.put('/api/db/inbox/order', {})).status).toBe(422);
  });
});

describe('/api/db/inbox ohne Board-Datenbank', () => {
  it('antwortet mit 503', async () => {
    const server = await TestServer.start();
    try {
      const res = await server.get('/api/db/inbox');
      expect(res.status).toBe(503);
      expect(res.json.code).toBe('board-config-missing');
    } finally {
      await server.dispose();
    }
  });
});
