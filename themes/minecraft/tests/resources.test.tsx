// @vitest-environment jsdom
// Ressourcen-Lader: Versionsliste → Versions-JSON → Client-JAR → Einträge, alles über den Asset-Proxy (fetch gemockt, kein Netz)
import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { themeAssetUrl } from '@nexus/theme-sdk';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  MINECRAFT_ENTRIES,
  MINECRAFT_VERSION,
  MANIFEST_URL,
  THEME_ID,
  resolveClientJar,
  useMinecraftResources,
} from '../src/resources.js';

const VERSION_JSON = 'https://piston-meta.mojang.com/v1/packages/abc/1.21.4.json';
const JAR = 'https://piston-data.mojang.com/v1/objects/def/client.jar';

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json', 'X-Nexus-Cache': 'miss' } });
}

// Proxy-Attrappe: antwortet je nach angefragter Mojang-URL; Einträge mit Platzhalter-Bytes
function proxy(overrides: (url: string) => Response | Promise<Response> | undefined = () => undefined) {
  return vi.fn(async (input: RequestInfo | URL) => {
    const url = String(input);
    const custom = await overrides(url);
    if (custom) return custom;
    if (url === themeAssetUrl(THEME_ID, MANIFEST_URL)) return json({ versions: [{ id: MINECRAFT_VERSION, url: VERSION_JSON }] });
    if (url === themeAssetUrl(THEME_ID, VERSION_JSON)) return json({ downloads: { client: { url: JAR } } });
    if (url.includes('entry=')) return new Response(new Uint8Array([1, 2, 3]), { status: 200, headers: { 'X-Nexus-Cache': 'hit' } });
    return json({ code: 'unexpected', message: url }, 500);
  });
}

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe('resolveClientJar', () => {
  it('liest Versionsliste und Versions-JSON über den Proxy und liefert die JAR-URL', async () => {
    const fetchMock = proxy();
    vi.stubGlobal('fetch', fetchMock);
    vi.spyOn(console, 'info').mockImplementation(() => {});
    expect(await resolveClientJar(THEME_ID, MINECRAFT_VERSION)).toBe(JAR);
    expect(fetchMock.mock.calls.map((c) => String(c[0]))).toEqual([
      themeAssetUrl(THEME_ID, MANIFEST_URL),
      themeAssetUrl(THEME_ID, VERSION_JSON),
    ]);
  });

  it('fehlende Version ergibt version_not_found', async () => {
    vi.stubGlobal('fetch', proxy());
    vi.spyOn(console, 'info').mockImplementation(() => {});
    await expect(resolveClientJar(THEME_ID, '0.0.0')).rejects.toMatchObject({ code: 'version_not_found' });
  });
});

describe('MINECRAFT_ENTRIES', () => {
  it('enthält Schrift-Atlas, Button-Texturen und Panorama 0–5', () => {
    expect([...MINECRAFT_ENTRIES]).toEqual([
      'assets/minecraft/textures/font/ascii.png',
      'assets/minecraft/textures/gui/sprites/widget/button.png',
      'assets/minecraft/textures/gui/sprites/widget/button_highlighted.png',
      ...[0, 1, 2, 3, 4, 5].map((i) => `assets/minecraft/textures/gui/title/background/panorama_${i}.png`),
    ]);
  });
});

describe('useMinecraftResources', () => {
  it('lädt alle Einträge nacheinander per entry, Fortschritt steigt bis ready', async () => {
    const fetchMock = proxy();
    vi.stubGlobal('fetch', fetchMock);
    const log = vi.spyOn(console, 'info').mockImplementation(() => {});
    const { result } = renderHook(() => useMinecraftResources());
    expect(result.current.phase).toBe('manifest');
    expect(result.current.total).toBe(MINECRAFT_ENTRIES.length);

    const seen: number[] = [];
    await waitFor(() => {
      seen.push(result.current.done);
      expect(result.current.phase).toBe('ready');
    });
    expect(result.current.done).toBe(MINECRAFT_ENTRIES.length);
    expect([...seen].sort((a, b) => a - b)).toEqual(seen);
    for (const entry of MINECRAFT_ENTRIES) {
      const url = themeAssetUrl(THEME_ID, JAR, { entry });
      expect(result.current.urls[entry]).toBe(url);
      expect(fetchMock).toHaveBeenCalledWith(url, expect.anything());
    }
    expect(result.current.error).toBeNull();
    expect(log).toHaveBeenCalledWith(expect.stringContaining('[minecraft]'), expect.objectContaining({ cache: 'miss' }));
    expect(log).toHaveBeenCalledWith(expect.stringContaining('[minecraft]'), expect.objectContaining({ cache: 'hit' }));
  });

  it('fragt höchstens einen Eintrag gleichzeitig an', async () => {
    let open = 0;
    let max = 0;
    vi.stubGlobal(
      'fetch',
      proxy(async (url) => {
        if (!url.includes('entry=')) return undefined;
        open++;
        max = Math.max(max, open);
        await new Promise((r) => setTimeout(r, 1));
        open--;
        return new Response(new Uint8Array([1]), { status: 200 });
      }),
    );
    vi.spyOn(console, 'info').mockImplementation(() => {});
    const { result } = renderHook(() => useMinecraftResources());
    await waitFor(() => expect(result.current.phase).toBe('ready'));
    expect(max).toBe(1);
  });

  it('Netzfehler: error mit offline, retry lädt neu', async () => {
    let fail = true;
    vi.stubGlobal(
      'fetch',
      proxy(() => (fail ? Promise.reject(new TypeError('Failed to fetch')) : undefined)),
    );
    vi.spyOn(console, 'info').mockImplementation(() => {});
    const { result } = renderHook(() => useMinecraftResources());
    await waitFor(() => expect(result.current.phase).toBe('error'));
    expect(result.current.error).toMatchObject({ offline: true, code: 'network' });

    fail = false;
    act(() => result.current.retry());
    await waitFor(() => expect(result.current.phase).toBe('ready'));
  });

  it('502 theme_fetch_upstream gilt als offline', async () => {
    vi.stubGlobal('fetch', proxy(() => json({ code: 'theme_fetch_upstream', message: 'upstream' }, 502)));
    vi.spyOn(console, 'info').mockImplementation(() => {});
    const { result } = renderHook(() => useMinecraftResources());
    await waitFor(() => expect(result.current.phase).toBe('error'));
    expect(result.current.error).toMatchObject({ offline: true, status: 502 });
  });

  it('403 nennt die fehlende Domain-Freigabe', async () => {
    vi.stubGlobal('fetch', proxy(() => json({ code: 'theme_fetch_forbidden', message: 'piston-meta.mojang.com' }, 403)));
    vi.spyOn(console, 'info').mockImplementation(() => {});
    const { result } = renderHook(() => useMinecraftResources());
    await waitFor(() => expect(result.current.phase).toBe('error'));
    expect(result.current.error).toMatchObject({ offline: false, status: 403, code: 'theme_fetch_forbidden' });
    expect(result.current.error?.message).toContain('Domain-Freigabe');
  });
});
