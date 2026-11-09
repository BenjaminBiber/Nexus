// @vitest-environment jsdom
// Asset-Index: Versions-JSON → Index-JSON → resources.download.minecraft.net, alles über den Asset-Proxy (fetch gemockt, kein Netz)
import { themeAssetUrl } from '@nexus/theme-sdk';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { RESOURCES_URL, resolveIndexUrls } from '../src/assetIndex.js';

const INDEX = 'https://piston-meta.mojang.com/v1/packages/xyz/19.json';
const HASH = 'a'.repeat(40);

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe('resolveIndexUrls', () => {
  it('baut je Schlüssel die Proxy-URL aus dem Hash, fehlende Einträge fehlen', async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      expect(String(input)).toBe(themeAssetUrl('minecraft', INDEX));
      return new Response(JSON.stringify({ objects: { 'a/b.png': { hash: HASH } } }), { status: 200 });
    });
    vi.stubGlobal('fetch', fetchMock);
    const urls = await resolveIndexUrls('minecraft', { assetIndex: { url: INDEX } }, { bild: 'a/b.png', fehlt: 'x/y.png' });
    expect(urls).toEqual({ bild: themeAssetUrl('minecraft', `${RESOURCES_URL}/aa/${HASH}`) });
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it('liefert leer ohne Index, bei ungültigem Hash, bei Fehlerantwort und bei Netzfehler', async () => {
    expect(await resolveIndexUrls('minecraft', {}, { bild: 'a/b.png' })).toEqual({});

    vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({ objects: { 'a/b.png': { hash: 'kurz' } } }))));
    expect(await resolveIndexUrls('minecraft', { assetIndex: { url: INDEX } }, { bild: 'a/b.png' })).toEqual({});

    vi.stubGlobal('fetch', vi.fn(async () => new Response('nein', { status: 502 })));
    expect(await resolveIndexUrls('minecraft', { assetIndex: { url: INDEX } }, { bild: 'a/b.png' })).toEqual({});

    vi.stubGlobal(
      'fetch',
      vi.fn(async () => {
        throw new Error('offline');
      }),
    );
    expect(await resolveIndexUrls('minecraft', { assetIndex: { url: INDEX } }, { bild: 'a/b.png' })).toEqual({});
  });
});
