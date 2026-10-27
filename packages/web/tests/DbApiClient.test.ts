import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiClient } from '../src/api/ApiClient.js';
import { ApiError } from '../src/api/ApiError.js';

afterEach(() => vi.unstubAllGlobals());

// Ersetzt fetch und merkt sich Methode, Adresse, Body und Köpfe jedes Aufrufs
function stubFetch(status = 200, body: unknown = {}) {
  const calls: Array<{ url: string; method: string; body: unknown; headers: Record<string, string> }> = [];
  vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
    calls.push({ url, method: init.method ?? 'GET', body: init.body ? JSON.parse(init.body as string) : undefined, headers: (init.headers ?? {}) as Record<string, string> });
    return new Response(JSON.stringify(body), { status });
  }));
  return calls;
}

describe('ApiClient für das DB-Board', () => {
  it('spricht die Routen unter /api/db an', async () => {
    const calls = stubFetch();
    const api = new ApiClient();
    await api.getDbActive();
    await api.getDbBoards('ARC');
    await api.getDbBoardView(3);
    await api.moveDbTicket('ARC-0001', { status: 'done', rev: 2, acceptance: 'ok' });
    await api.patchDbTicket('ARC-0001', { rev: 2, title: 'Neu' });
    await api.deleteDbBoard(3, true);
    await api.getDbNotes(3, 'all');
    await api.dropDbNote(5, 'doppelt');
    await api.activateDbBoard(2);
    await api.setDbStep('ARC-0001', 2, true);
    expect(calls.map((c) => `${c.method} ${c.url}`)).toEqual([
      'GET /api/db/active', 'GET /api/db/boards?project=ARC', 'GET /api/db/boards/3/view',
      'POST /api/db/tickets/ARC-0001/move', 'PATCH /api/db/tickets/ARC-0001', 'DELETE /api/db/boards/3?deleteOpen=true',
      'GET /api/db/boards/3/notes?state=all', 'POST /api/db/notes/5/drop', 'POST /api/db/boards/2/activate',
      'POST /api/db/tickets/ARC-0001/steps/2',
    ]);
    expect(calls[3].body).toEqual({ status: 'done', rev: 2, acceptance: 'ok' });
    expect(calls[3].headers['x-harness']).toBe('1');
    expect(calls[4].body).toEqual({ rev: 2, title: 'Neu' });
    expect(calls[7].body).toEqual({ reason: 'doppelt' });
    expect(calls[9].body).toEqual({ done: true });
  });

  it('liest Zeitleiste und Suche', async () => {
    const calls = stubFetch();
    const api = new ApiClient();
    await api.getDbTicketTimeline('ARC-0001');
    await api.searchDb('a&b ä', { kinds: ['ticket', 'note'], limit: 20 });
    await api.searchDb('plan');
    expect(calls.map((c) => `${c.method} ${c.url}`)).toEqual([
      'GET /api/db/tickets/ARC-0001/timeline',
      'GET /api/db/search?q=a%26b+%C3%A4&kinds=ticket%2Cnote&limit=20',
      'GET /api/db/search?q=plan',
    ]);
  });

  it('holt Tickets von einem anderen Board', async () => {
    const calls = stubFetch(200, { moved: [] });
    const result = await new ApiClient().importDbTickets(3, ['ARC-0001', 'ARC-0002']);
    expect(calls.map((c) => `${c.method} ${c.url}`)).toEqual(['POST /api/db/boards/3/import']);
    expect(calls[0].body).toEqual({ ticketIds: ['ARC-0001', 'ARC-0002'] });
    expect(calls[0].headers['x-harness']).toBe('1');
    expect(result).toEqual({ moved: [] });
  });

  it('meldet eine nicht verfügbare Board-Datenbank als ApiError mit Status 503', async () => {
    stubFetch(503, { code: 'board-db-unreachable', message: 'nicht erreichbar' });
    const error = await new ApiClient().getDbActive().catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect(error).toMatchObject({ code: 'board-db-unreachable', status: 503, message: 'nicht erreichbar' });
  });

  it('schreibt einen Dokumentabschnitt per PUT samt Hash', async () => {
    const calls = stubFetch(200, { path: 'docs/superpowers/specs/a.md', hash: 'neu', sections: [] });
    const input = { path: 'docs/superpowers/specs/a.md', startLine: 5, endLine: 7, content: '## Ziel', expectedHash: 'alt' };
    const result = await new ApiClient().writeDocSection('p1', input);
    expect(calls.map((c) => `${c.method} ${c.url}`)).toEqual(['PUT /api/projects/p1/docs/section']);
    expect(calls[0].body).toEqual(input);
    expect(calls[0].headers['x-harness']).toBe('1');
    expect(result).toEqual({ path: 'docs/superpowers/specs/a.md', hash: 'neu', sections: [] });
  });

  it('hängt das beobachtete Board an den Ereignisstrom', () => {
    const urls: string[] = [];
    vi.stubGlobal('EventSource', class {
      constructor(url: string) {
        urls.push(url);
      }
    });
    const api = new ApiClient();
    api.openEvents('p1');
    api.openEvents('p1', 4);
    expect(urls).toEqual(['/api/projects/p1/events', '/api/projects/p1/events?dbBoard=4']);
  });
});
