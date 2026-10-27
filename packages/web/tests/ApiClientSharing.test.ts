import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiClient } from '../src/api/ApiClient.js';

afterEach(() => vi.unstubAllGlobals());

// Ersetzt fetch; jede Antwort wartet, bis release() aufgerufen wird
function stubSlowFetch(body: unknown) {
  const urls: string[] = [];
  const pending: Array<() => void> = [];
  vi.stubGlobal('fetch', vi.fn((url: string, init: RequestInit) => {
    urls.push(`${init.method ?? 'GET'} ${url}`);
    return new Promise<Response>((resolve) => pending.push(() => resolve(new Response(JSON.stringify(body)))));
  }));
  return { urls, release: () => pending.splice(0).forEach((r) => r()) };
}

describe('ApiClient teilt gleichzeitige GETs', () => {
  it('schickt gleichzeitige gleiche GETs nur einmal und gibt jedem eine eigene Kopie', async () => {
    const fetch = stubSlowFetch({ boardId: 1, list: [1] });
    const api = new ApiClient();
    const first = api.getDbActive();
    const second = api.getDbActive();
    fetch.release();
    const [a, b] = await Promise.all([first, second]);
    expect(fetch.urls).toEqual(['GET /api/db/active']);
    expect(a).toEqual(b);
    expect(a).not.toBe(b);
  });

  it('fragt nach Abschluss erneut', async () => {
    const fetch = stubSlowFetch({});
    const api = new ApiClient();
    const first = api.getDbActive();
    fetch.release();
    await first;
    const second = api.getDbActive();
    fetch.release();
    await second;
    expect(fetch.urls).toEqual(['GET /api/db/active', 'GET /api/db/active']);
  });

  it('teilt kein GET über eine schreibende Anfrage hinweg', async () => {
    const fetch = stubSlowFetch({});
    const api = new ApiClient();
    const before = api.getDbActive();
    const write = api.activateDbBoard(2);
    const after = api.getDbActive();
    fetch.release();
    await Promise.all([before, write, after]);
    expect(fetch.urls).toEqual(['GET /api/db/active', 'POST /api/db/boards/2/activate', 'GET /api/db/active']);
  });
});
