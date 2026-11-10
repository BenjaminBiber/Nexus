import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiClient } from '../src/api/ApiClient.js';

afterEach(() => vi.unstubAllGlobals());

// Ersetzt fetch und merkt sich Methode, Pfad und Körper
function stubFetch(body: unknown, status = 200) {
  const calls: Array<{ method: string; url: string; body: unknown }> = [];
  vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
    calls.push({ method: init.method ?? 'GET', url, body: init.body === undefined ? undefined : JSON.parse(init.body as string) });
    return status === 204 ? new Response(null, { status }) : new Response(JSON.stringify(body), { status });
  }));
  return calls;
}

describe('ApiClient: Store-Repos', () => {
  it('listet die Repos', async () => {
    const calls = stubFetch([{ id: 'a', url: 'https://x/y.json', addedAt: '' }]);
    const repos = await new ApiClient().listStoreRepos();
    expect(calls[0]).toMatchObject({ method: 'GET', url: '/api/themes/store/repos' });
    expect(repos[0].id).toBe('a');
  });

  it('trägt ein Repo mit URL ein', async () => {
    const calls = stubFetch({ id: 'a', url: 'https://x/y.json', addedAt: '' }, 201);
    await new ApiClient().addStoreRepo('https://x/y.json');
    expect(calls[0]).toEqual({ method: 'POST', url: '/api/themes/store/repos', body: { url: 'https://x/y.json' } });
  });

  it('entfernt ein Repo mit kodierter ID', async () => {
    const calls = stubFetch(null, 204);
    await new ApiClient().removeStoreRepo('a/b');
    expect(calls[0]).toMatchObject({ method: 'DELETE', url: '/api/themes/store/repos/a%2Fb' });
  });
});

describe('ApiClient: Store durchsuchen, installieren, aktualisieren', () => {
  it('lädt den Index', async () => {
    const calls = stubFetch({ entries: [], repos: [] });
    const index = await new ApiClient().getStoreIndex();
    expect(calls[0]).toMatchObject({ method: 'GET', url: '/api/themes/store/index' });
    expect(index.entries).toEqual([]);
  });

  it('installiert einen Eintrag mit Repo und ID', async () => {
    const calls = stubFetch({ manifest: {}, verified: false, repoId: 'r', sha256: 'x' });
    await new ApiClient().installFromStore('r', 'ocean');
    expect(calls[0]).toEqual({ method: 'POST', url: '/api/themes/store/install', body: { repoId: 'r', id: 'ocean' } });
  });

  it('lädt die Updates', async () => {
    const calls = stubFetch({ updates: [], errors: [] });
    await new ApiClient().getStoreUpdates();
    expect(calls[0]).toMatchObject({ method: 'GET', url: '/api/themes/store/updates' });
  });

  it('aktualisiert ohne confirm, mit confirm nur bei Bestätigung', async () => {
    const calls = stubFetch({ manifest: {}, verified: false, repoId: 'r', sha256: 'x', previousVersion: '1.0.0' });
    await new ApiClient().updateFromStore('r', 'ocean');
    await new ApiClient().updateFromStore('r', 'ocean', true);
    expect(calls[0]).toEqual({ method: 'POST', url: '/api/themes/store/update', body: { repoId: 'r', id: 'ocean' } });
    expect(calls[1].body).toEqual({ repoId: 'r', id: 'ocean', confirm: true });
  });

  it('liefert bei fehlender Bestätigung hasCode und newDomains in details', async () => {
    stubFetch({ code: 'store-update-confirmation-required', message: 'Bitte bestätigen', details: { hasCode: true, newDomains: ['a.example'] } }, 409);
    const err = await new ApiClient().updateFromStore('r', 'ocean').catch((e) => e);
    expect(err.code).toBe('store-update-confirmation-required');
    expect(err.details).toEqual({ hasCode: true, newDomains: ['a.example'] });
  });
});
