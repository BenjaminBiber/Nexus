// @vitest-environment jsdom
// Server-Adresse und Token: im Browser gleiche Herkunft, in der Desktop-App aus window.__NEXUS__
import { afterEach, describe, expect, it, vi } from 'vitest';
import { apiBase, apiHeaders, boardViewUrl, eventsUrl, fetchBoardView } from '../src/data/api.js';

type Bridged = Window & { __NEXUS__?: { base: string; token: string } };

afterEach(() => {
  delete (window as Bridged).__NEXUS__;
  vi.unstubAllGlobals();
});

const bridge = () => ((window as Bridged).__NEXUS__ = { base: 'http://127.0.0.1:4711', token: 'a b&c' });

describe('Adressen ohne Desktop-App', () => {
  it('gleiche Herkunft, kein Token', () => {
    expect(apiBase()).toBe('');
    expect(apiHeaders()).toEqual({});
    expect(boardViewUrl(3)).toBe('/api/db/boards/3/view');
    expect(eventsUrl('p1', 3)).toBe('/api/projects/p1/events?dbBoard=3');
  });

  it('nimmt eine unvollständige Brücke nicht an', () => {
    (window as Bridged).__NEXUS__ = { base: 'http://x', token: '' };
    expect(apiBase()).toBe('');
    expect(apiHeaders()).toEqual({});
  });
});

describe('Adressen in der Desktop-App', () => {
  it('Basisadresse, Token im Kopf und in der Adresse des Ereignisstroms', () => {
    bridge();
    expect(apiBase()).toBe('http://127.0.0.1:4711');
    expect(apiHeaders()).toEqual({ 'x-nexus-token': 'a b&c' });
    expect(boardViewUrl(3)).toBe('http://127.0.0.1:4711/api/db/boards/3/view');
    expect(eventsUrl('p 1', 3)).toBe('http://127.0.0.1:4711/api/projects/p%201/events?dbBoard=3&token=a%20b%26c');
  });
});

describe('fetchBoardView', () => {
  const view = { boardId: 3, columns: [{ id: 'done', title: 'Erledigt', tickets: [] }] };

  it('lädt den Board-Stand mit Token und Signal', async () => {
    bridge();
    const fetchMock = vi.fn(async () => new Response(JSON.stringify(view), { status: 200 }));
    vi.stubGlobal('fetch', fetchMock);
    const signal = new AbortController().signal;
    expect(await fetchBoardView(3, signal)).toEqual(view);
    expect(fetchMock).toHaveBeenCalledWith('http://127.0.0.1:4711/api/db/boards/3/view', { headers: { 'x-nexus-token': 'a b&c' }, signal });
  });

  it('liefert null statt zu werfen: Netzfehler, Fehlerstatus, kein JSON, Unsinn', async () => {
    const answers = [
      () => Promise.reject(new TypeError('offline')),
      async () => new Response('{}', { status: 500 }),
      async () => new Response('kein json', { status: 200 }),
      async () => new Response(JSON.stringify({ quatsch: true }), { status: 200 }),
    ];
    for (const answer of answers) {
      vi.stubGlobal('fetch', vi.fn(answer));
      expect(await fetchBoardView(3, new AbortController().signal)).toBeNull();
    }
  });
});
