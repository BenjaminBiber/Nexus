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
