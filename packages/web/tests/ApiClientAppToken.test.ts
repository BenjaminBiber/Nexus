import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiClient } from '../src/api/ApiClient.js';
import { nexusBridge } from '../src/api/nexusBridge.js';

afterEach(() => vi.unstubAllGlobals());

// Ersetzt fetch und merkt sich Adresse und Köpfe jeder Anfrage
function stubFetch(body: unknown = {}) {
  const calls: Array<{ url: string; method: string; headers: Record<string, string> }> = [];
  vi.stubGlobal('fetch', vi.fn((url: string, init: RequestInit) => {
    calls.push({ url, method: init.method ?? 'GET', headers: (init.headers ?? {}) as Record<string, string> });
    return Promise.resolve(new Response(JSON.stringify(body)));
  }));
  return calls;
}

// Ersetzt EventSource und merkt sich die Adressen
function stubEventSource() {
  const urls: string[] = [];
  vi.stubGlobal('EventSource', class {
    constructor(url: string) {
      urls.push(url);
    }
  });
  return urls;
}

describe('ApiClient in der Desktop-App', () => {
  const BASE = 'http://127.0.0.1:4810';
  const TOKEN = 'geheimes-token';

  it('schickt den Kopf X-Nexus-Token bei lesenden und schreibenden Anfragen', async () => {
    const calls = stubFetch();
    const api = new ApiClient(BASE, TOKEN);

    await api.getDbActive();
    await api.activateDbBoard(2);

    expect(calls[0].url).toBe(`${BASE}/api/db/active`);
    expect(calls[0].headers['x-nexus-token']).toBe(TOKEN);
    expect(calls[1].url).toBe(`${BASE}/api/db/boards/2/activate`);
    expect(calls[1].headers['x-nexus-token']).toBe(TOKEN);
    // Der Pflichtkopf des Browsers bleibt dabei, der Server verlangt ihn von der App nur nicht
    expect(calls[1].headers['x-harness']).toBe('1');
    expect(calls[1].headers['content-type']).toBe('application/json');
  });

  it('schickt den Kopf auch beim Abruf eines Protokolls', async () => {
    const calls = stubFetch();
    vi.stubGlobal('fetch', vi.fn((url: string, init: RequestInit) => {
      calls.push({ url, method: init.method ?? 'GET', headers: (init.headers ?? {}) as Record<string, string> });
      return Promise.resolve(new Response('Zeile eins'));
    }));

    const text = await new ApiClient(BASE, TOKEN).getDispatchLog('r1.log');

    expect(text).toBe('Zeile eins');
    expect(calls[0].url).toBe(`${BASE}/api/dispatch/logs/r1.log`);
    expect(calls[0].headers['x-nexus-token']).toBe(TOKEN);
  });

  it('hängt das Token an die Adresse des Ereignisstroms', () => {
    const urls = stubEventSource();
    const api = new ApiClient(BASE, TOKEN);

    api.openEvents('p1');
    api.openEvents('p1', 3, 4, true);

    expect(urls[0]).toBe(`${BASE}/api/projects/p1/events?token=${TOKEN}`);
    expect(urls[1]).toBe(`${BASE}/api/projects/p1/events?dbBoard=3&dbProject=4&notify=1&token=${TOKEN}`);
  });
});

describe('ApiClient im Browser', () => {
  it('bleibt ohne Token unverändert', async () => {
    const calls = stubFetch();
    const urls = stubEventSource();
    const api = new ApiClient();

    await api.getDbActive();
    await api.activateDbBoard(2);
    api.openEvents('p1', 3);

    expect(calls[0].url).toBe('/api/db/active');
    expect(calls[0].headers).toEqual({});
    expect(calls[1].headers).toEqual({ 'content-type': 'application/json', 'x-harness': '1' });
    expect(urls[0]).toBe('/api/projects/p1/events?dbBoard=3');
  });
});

describe('nexusBridge', () => {
  it('liefert die Werte der App, sobald Adresse und Token da sind', () => {
    vi.stubGlobal('window', { __NEXUS__: { base: 'http://127.0.0.1:4810', token: 'geheim' } });
    expect(nexusBridge()).toEqual({ base: 'http://127.0.0.1:4810', token: 'geheim' });
  });

  it('liefert im Browser nichts', () => {
    vi.stubGlobal('window', {});
    expect(nexusBridge()).toBeUndefined();
  });

  it('liefert nichts, wenn Adresse oder Token fehlen', () => {
    vi.stubGlobal('window', { __NEXUS__: { base: 'http://127.0.0.1:4810', token: '' } });
    expect(nexusBridge()).toBeUndefined();
    vi.stubGlobal('window', { __NEXUS__: { base: '', token: 'geheim' } });
    expect(nexusBridge()).toBeUndefined();
  });
});
