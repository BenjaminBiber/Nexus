import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';

describe('DbTicketRoutes', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;
  let boardId: number;

  beforeEach(async () => {
    ({ server, backend } = await TestServer.startWithBoard());
    await backend.services.projects.create('ARC', 'Arches', backend.actor());
    boardId = (await backend.services.boards.create('ARC', 'Sprint', backend.actor())).id;
  });
  afterEach(() => server.dispose());

  it('legt ein Ticket an und liefert es mit Kommentaren und Verlauf', async () => {
    const created = await server.post(`/api/db/boards/${boardId}/tickets`, { title: 'Login', description: 'Text', priority: 'high', labels: ['ui'], needsPlanning: false });
    expect(created.status).toBe(201);
    expect(created.json).toMatchObject({ id: 'ARC-0001', title: 'Login', priority: 'high', labels: ['ui'], needsPlanning: false, column: 'backlog', rev: 1 });
    const detail = await server.get('/api/db/tickets/ARC-0001');
    expect(detail.json).toMatchObject({ id: 'ARC-0001', comments: [], history: [expect.objectContaining({ note: 'angelegt', byName: 'Florian' })] });
    expect((await server.get('/api/db/tickets/ARC-0099')).status).toBe(404);
    expect((await server.post(`/api/db/boards/${boardId}/tickets`, { title: ' ' })).json.code).toBe('title-empty');
    expect((await server.post('/api/db/boards/999/tickets', { title: 'A' })).status).toBe(404);
  });

  it('lehnt ein offenes Ticket mit gleichem Titel mit 422 ab', async () => {
    await server.post(`/api/db/boards/${boardId}/tickets`, { title: 'Login' });
    const dup = await server.post(`/api/db/boards/${boardId}/tickets`, { title: ' login ' });
    expect(dup.status).toBe(422);
    expect(dup.json).toMatchObject({ code: 'ticket-duplicate', message: 'Ein offenes Ticket mit diesem Titel gibt es schon: ARC-0001.' });
  });

  it('leert Branch und Worktree mit branch: null (beide, mit Verlauf); setzen lassen sie sich hier nicht', async () => {
    await server.post(`/api/db/boards/${boardId}/tickets`, { title: 'Login' });
    await backend.services.tickets.update('ARC-0001', { branch: 'ticket/ARC-0001-login', worktree: 'C:/w/ARC-0001' }, backend.actor());
    expect((await server.patch('/api/db/tickets/ARC-0001', { rev: 2, branch: 'anderer' })).json.code).toBe('input-invalid');
    const cleared = await server.patch('/api/db/tickets/ARC-0001', { rev: 2, branch: null });
    expect(cleared.status).toBe(200);
    expect(cleared.json).toMatchObject({ branch: null, worktree: null, rev: 3 });
    expect(cleared.json.history).toEqual(expect.arrayContaining([
      expect.objectContaining({ field: 'branch', from: 'ticket/ARC-0001-login', to: null }),
      expect.objectContaining({ field: 'worktree', from: 'C:/w/ARC-0001', to: null }),
    ]));
  });

  it('legt ein Ticket mit Zielbranch an, setzt und leert ihn per PATCH und lehnt ungültige Namen ab', async () => {
    const created = await server.post(`/api/db/boards/${boardId}/tickets`, { title: 'Login', targetBranch: ' release/1 ' });
    expect(created.status).toBe(201);
    expect(created.json).toMatchObject({ id: 'ARC-0001', targetBranch: 'release/1' });
    expect((await server.post(`/api/db/boards/${boardId}/tickets`, { title: 'Ohne' })).json.targetBranch).toBeNull();
    const changed = await server.patch('/api/db/tickets/ARC-0002', { rev: 1, targetBranch: 'release/2' });
    expect(changed.json).toMatchObject({ targetBranch: 'release/2', rev: 2 });
    expect(changed.json.history).toEqual(expect.arrayContaining([expect.objectContaining({ field: 'targetBranch', from: null, to: 'release/2' })]));
    const cleared = await server.patch('/api/db/tickets/ARC-0001', { rev: 1, targetBranch: null });
    expect(cleared.status).toBe(200);
    expect(cleared.json).toMatchObject({ targetBranch: null, rev: 2 });
    const invalid = await server.patch('/api/db/tickets/ARC-0001', { rev: 2, targetBranch: 'ticket/x' });
    expect(invalid.status).toBe(422);
    expect(invalid.json.code).toBe('target-branch-invalid');
    expect((await server.post(`/api/db/boards/${boardId}/tickets`, { title: 'Falsch', targetBranch: 'a b' })).json.code).toBe('target-branch-invalid');
    expect((await server.post(`/api/db/boards/${boardId}/tickets`, { title: 'Zahl', targetBranch: 5 })).json.code).toBe('input-invalid');
  });

  it('ändert Felder mit Version und meldet einen Konflikt mit dem aktuellen Stand', async () => {
    await server.post(`/api/db/boards/${boardId}/tickets`, { title: 'Login' });
    const saved = await server.patch('/api/db/tickets/ARC-0001', { rev: 1, title: 'Login bauen', plan: '# Plan', assignee: 'claude', labels: ['a', 'b'] });
    expect(saved.status).toBe(200);
    expect(saved.json).toMatchObject({ title: 'Login bauen', plan: '# Plan', assignee: 'claude', labels: ['a', 'b'], rev: 2, comments: [] });
    const stale = await server.patch('/api/db/tickets/ARC-0001', { rev: 1, title: 'Alt' });
    expect(stale.status).toBe(409);
    expect(stale.json).toMatchObject({ code: 'conflict', current: { id: 'ARC-0001', title: 'Login bauen', rev: 2 } });
    expect((await server.patch('/api/db/tickets/ARC-0001', { title: 'ohne Version' })).json.code).toBe('rev-missing');
    expect((await server.patch('/api/db/tickets/ARC-0001', { rev: 2 })).json.code).toBe('nothing-changed');
    expect((await server.patch('/api/db/tickets/ARC-0001', { rev: 2, status: 'quatsch' })).json.code).toBe('status-invalid');
    expect((await server.patch('/api/db/tickets/ARC-0001', { rev: 2, assignee: null })).json.assignee).toBeNull();
  });

  it('verschiebt als Nutzer auch nach „Nicht mehr nötig“ und prüft Abnahme und Version', async () => {
    await server.post(`/api/db/boards/${boardId}/tickets`, { title: 'A' });
    const done = await server.post('/api/db/tickets/ARC-0001/move', { status: 'done' });
    expect(done.status).toBe(422);
    expect(done.json.code).toBe('acceptance-missing');
    expect((await server.post('/api/db/tickets/ARC-0001/move', { status: 'done', acceptance: 'geht' })).json).toMatchObject({ status: 'done', acceptance: 'geht', rev: 2 });
    const moved = await server.post('/api/db/tickets/ARC-0001/move', { status: 'not_needed', rev: 2 });
    expect(moved.json).toMatchObject({ status: 'not_needed', column: 'not_needed', rev: 3 });
    const stale = await server.post('/api/db/tickets/ARC-0001/move', { status: 'backlog', rev: 1 });
    expect(stale.status).toBe(409);
    expect(stale.json.current.rev).toBe(3);
    expect((await server.post('/api/db/tickets/ARC-0001/move', {})).json.code).toBe('status-missing');
  });

  it('hängt Kommentare an und löscht Tickets', async () => {
    await server.post(`/api/db/boards/${boardId}/tickets`, { title: 'A' });
    const commented = await server.post('/api/db/tickets/ARC-0001/comments', { body: 'Hallo' });
    expect(commented.status).toBe(201);
    expect(commented.json.comments).toEqual([expect.objectContaining({ body: 'Hallo', authorName: 'Florian' })]);
    expect((await server.post('/api/db/tickets/ARC-0001/comments', { body: ' ' })).json.code).toBe('comment-empty');
    expect((await server.delete('/api/db/tickets/ARC-0001')).status).toBe(204);
    expect((await server.get('/api/db/tickets/ARC-0001')).status).toBe(404);
  });

  it('ergänzt Voraussetzungen und lehnt Kreise ab', async () => {
    await server.post(`/api/db/boards/${boardId}/tickets`, { title: 'A' });
    await server.post(`/api/db/boards/${boardId}/tickets`, { title: 'B', dependsOn: ['ARC-0001'] });
    const cycle = await server.patch('/api/db/tickets/ARC-0001', { rev: 1, dependsOn: ['ARC-0002'] });
    expect(cycle.status).toBe(422);
    expect(cycle.json.code).toBe('dependency-cycle');
  });

  describe('Tickets von anderem Board holen', () => {
    let sourceId: number;

    beforeEach(async () => {
      sourceId = (await backend.services.boards.create('ARC', 'Kollege', backend.actor())).id;
      await server.post(`/api/db/boards/${sourceId}/tickets`, { title: 'A' });
      await server.post(`/api/db/boards/${sourceId}/tickets`, { title: 'B', status: 'planned' });
    });

    it('holt Tickets in einer Transaktion und ändert den Kennwert beider Boards', async () => {
      const markers = [await backend.db.changeMarker(sourceId), await backend.db.changeMarker(boardId)];
      const res = await server.post(`/api/db/boards/${boardId}/import`, { ticketIds: ['ARC-0001', 'arc-2'] });
      expect(res.status).toBe(200);
      expect(res.json.moved).toEqual([
        expect.objectContaining({ id: 'ARC-0001', boardId, rev: 2, column: 'backlog' }),
        expect.objectContaining({ id: 'ARC-0002', boardId, rev: 2, column: 'planned' }),
      ]);
      expect(await backend.db.changeMarker(sourceId)).not.toBe(markers[0]);
      expect(await backend.db.changeMarker(boardId)).not.toBe(markers[1]);
      const detail = await server.get('/api/db/tickets/ARC-0001');
      expect(detail.json.history.at(-1)).toMatchObject({ field: 'boardId', note: 'von Board Kollege geholt', byName: 'Florian' });
    });

    it('holt nichts, wenn ein Ticket fehlt, schon da ist oder aus einem anderen Projekt kommt', async () => {
      await server.post(`/api/db/boards/${boardId}/tickets`, { title: 'Hier' });
      await backend.services.projects.create('INC', 'Incidents', backend.actor());
      const foreign = (await backend.services.boards.create('INC', 'Fremd', backend.actor())).id;
      await server.post(`/api/db/boards/${foreign}/tickets`, { title: 'Fremd' });
      const missing = await server.post(`/api/db/boards/${boardId}/import`, { ticketIds: ['ARC-0001', 'ARC-0099'] });
      expect([missing.status, missing.json.code]).toEqual([404, 'ticket-not-found']);
      const here = await server.post(`/api/db/boards/${boardId}/import`, { ticketIds: ['ARC-0001', 'ARC-0003'] });
      expect([here.status, here.json.code]).toEqual([422, 'ticket-already-on-board']);
      const other = await server.post(`/api/db/boards/${boardId}/import`, { ticketIds: ['ARC-0001', 'INC-0001'] });
      expect([other.status, other.json.code]).toEqual([422, 'board-other-project']);
      expect((await server.get('/api/db/tickets/ARC-0001')).json).toMatchObject({ boardId: sourceId, rev: 1 });
      expect((await server.post('/api/db/boards/999/import', { ticketIds: ['ARC-0001'] })).status).toBe(404);
    });

    it('prüft den Body: nicht leere Liste von Texten, höchstens 200', async () => {
      const tooMany = Array.from({ length: 201 }, (_, i) => `ARC-${i + 1}`);
      for (const body of [{}, { ticketIds: [] }, { ticketIds: 'ARC-0001' }, { ticketIds: ['ARC-0001', 7] }, { ticketIds: tooMany }]) {
        const res = await server.post(`/api/db/boards/${boardId}/import`, body);
        expect(res.status, JSON.stringify(body).slice(0, 40)).toBe(422);
        expect(res.json.code).toBe('input-invalid');
      }
      expect((await server.get('/api/db/tickets/ARC-0001')).json.boardId).toBe(sourceId);
    });
  });

  it('hakt einen Schritt ab und zurück', async () => {
    await backend.services.tickets.create(boardId, { title: 'Mit Schritten', steps: [{ text: 'A', done: false }, { text: 'B', done: false }] }, backend.actor());
    const done = await server.post('/api/db/tickets/ARC-0001/steps/2', { done: true });
    expect(done.status).toBe(200);
    expect(done.json).toMatchObject({ stepsDone: 1, stepsTotal: 2, steps: [{ text: 'A', done: false }, { text: 'B', done: true }] });
    expect((await server.post('/api/db/tickets/ARC-0001/steps/2', { done: false })).json.stepsDone).toBe(0);
    expect((await server.post('/api/db/tickets/ARC-0001/steps/3', { done: true })).status).toBe(404);
    expect((await server.post('/api/db/tickets/ARC-0001/steps/0', { done: true })).json.code).toBe('input-invalid');
    expect((await server.post('/api/db/tickets/ARC-0001/steps/1', {})).json.code).toBe('input-invalid');
  });

  it('lehnt falsche Typen beim Abhaken mit 422 ab', async () => {
    await backend.services.tickets.create(boardId, { title: 'Mit Schritten', steps: [{ text: 'A', done: false }] }, backend.actor());
    for (const [path, body] of [['steps/1', { done: 'ja' }], ['steps/1', { done: 1 }], ['steps/1', { done: null }], ['steps/x', { done: true }], ['steps/1.5', { done: true }]] as const) {
      const res = await server.post(`/api/db/tickets/ARC-0001/${path}`, body);
      expect(res.status, path + JSON.stringify(body)).toBe(422);
      expect(res.json.code).toBe('input-invalid');
    }
  });

  it('legt Schritte an, bearbeitet, verschiebt und löscht sie und liefert das Ticketdetail', async () => {
    await backend.services.tickets.create(boardId, { title: 'Ohne Schritte' }, backend.actor());
    const added = await server.post('/api/db/tickets/ARC-0001/steps', { text: 'Test schreiben', detail: 'Erwartet: rot' });
    expect(added.status).toBe(201);
    expect(added.json).toMatchObject({ id: 'ARC-0001', rev: 2, stepsTotal: 1, steps: [{ text: 'Test schreiben', done: false, detail: 'Erwartet: rot' }], history: expect.any(Array) });
    await server.post('/api/db/tickets/ARC-0001/steps', { text: 'Lesen', at: 1 });
    const edited = await server.patch('/api/db/tickets/ARC-0001/steps/2', { text: 'Test zuerst', detail: null });
    expect(edited.status).toBe(200);
    expect(edited.json.steps).toEqual([{ text: 'Lesen', done: false }, { text: 'Test zuerst', done: false }]);
    const moved = await server.post('/api/db/tickets/ARC-0001/steps/2/move', { to: 1 });
    expect(moved.json.steps.map((s: { text: string }) => s.text)).toEqual(['Test zuerst', 'Lesen']);
    const removed = await server.delete('/api/db/tickets/ARC-0001/steps/1');
    expect(removed.status).toBe(200);
    expect(removed.json).toMatchObject({ stepsTotal: 1, steps: [{ text: 'Lesen' }] });
  });

  it('meldet Fehler beim Bearbeiten von Schritten', async () => {
    await backend.services.tickets.create(boardId, { title: 'A', steps: [{ text: 'A', done: false }] }, backend.actor());
    expect((await server.post('/api/db/tickets/ARC-0001/steps', { text: ' ' })).json.code).toBe('step-empty');
    expect((await server.post('/api/db/tickets/ARC-0001/steps', {})).status).toBe(422);
    expect((await server.post('/api/db/tickets/ARC-0001/steps', { text: 'X', at: 5 })).status).toBe(404);
    expect((await server.post('/api/db/tickets/ARC-0099/steps', { text: 'X' })).status).toBe(404);
    expect((await server.patch('/api/db/tickets/ARC-0001/steps/2', { text: 'X' })).status).toBe(404);
    expect((await server.patch('/api/db/tickets/ARC-0001/steps/1', {})).json.code).toBe('nothing-changed');
    expect((await server.patch('/api/db/tickets/ARC-0001/steps/x', { text: 'X' })).json.code).toBe('input-invalid');
    expect((await server.delete('/api/db/tickets/ARC-0001/steps/2')).status).toBe(404);
    expect((await server.post('/api/db/tickets/ARC-0001/steps/1/move', {})).json.code).toBe('input-invalid');
    expect((await server.post('/api/db/tickets/ARC-0001/steps/1/move', { to: 2 })).status).toBe(404);
  });

  it('meldet geänderte Schritte (expectText) und veraltete Versionen (rev) mit 409 und dem aktuellen Stand', async () => {
    await backend.services.tickets.create(boardId, { title: 'A', steps: [{ text: 'A', done: false }, { text: 'B', done: false }] }, backend.actor());
    await server.post('/api/db/tickets/ARC-0001/steps/2/move', { to: 1 });
    const checks = [
      await server.post('/api/db/tickets/ARC-0001/steps/1', { done: true, expectText: 'A' }),
      await server.post('/api/db/tickets/ARC-0001/steps', { text: 'X', at: 1, expectText: 'A' }),
      await server.patch('/api/db/tickets/ARC-0001/steps/1', { text: 'X', expectText: 'A' }),
      await server.request('DELETE', '/api/db/tickets/ARC-0001/steps/1', { expectText: 'A' }),
      await server.post('/api/db/tickets/ARC-0001/steps/1/move', { to: 2, expectText: 'A' }),
      await server.patch('/api/db/tickets/ARC-0001/steps/1', { text: 'X', rev: 1 }),
    ];
    for (const res of checks) {
      expect(res.status).toBe(409);
      expect(res.json).toMatchObject({ code: 'conflict', current: { id: 'ARC-0001', rev: 2, steps: [{ text: 'B' }, { text: 'A' }] } });
    }
    expect(checks[0].json.message).toBe('ARC-0001: Schritt 1 ist inzwischen „B“ – die Schritte wurden geändert.');
    const ok = await server.request('DELETE', '/api/db/tickets/ARC-0001/steps/1', { expectText: 'B', rev: 2 });
    expect(ok.status).toBe(200);
    expect(ok.json.steps).toEqual([{ text: 'A', done: false }]);
    const added = await server.post('/api/db/tickets/ARC-0001/steps', { text: 'C', rev: 3 });
    expect(added.status).toBe(201);
  });
});
