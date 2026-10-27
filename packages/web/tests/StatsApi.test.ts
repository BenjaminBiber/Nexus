import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiClient } from '../src/api/ApiClient.js';

afterEach(() => vi.unstubAllGlobals());

describe('ApiClient für das Dashboard', () => {
  it('baut die Abfrage von /api/db/stats/dispatch mit Board, Zeitraum und Lauf-Art', async () => {
    const urls: string[] = [];
    vi.stubGlobal('fetch', vi.fn(async (url: string) => {
      urls.push(url);
      return new Response('{}', { status: 200 });
    }));
    const api = new ApiClient();
    await api.getDispatchStats({ project: 'ARC' });
    await api.getDispatchStats({ project: 'A&B', board: 3, from: '2026-01-01', to: '2026-02-01', kind: 'merge' });
    await api.getDispatchStats({ project: 'ARC', board: null, from: 'config' });
    expect(urls).toEqual([
      '/api/db/stats/dispatch?project=ARC',
      '/api/db/stats/dispatch?project=A%26B&board=3&from=2026-01-01&to=2026-02-01&kind=merge',
      '/api/db/stats/dispatch?project=ARC&from=config',
    ]);
  });
});
