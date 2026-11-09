// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, renderHook, screen, waitFor } from '@testing-library/react';
import { ThemeHostProvider, themeAssetUrl, useThemeAsset } from '@nexus/theme-sdk';
import type { ThemeAssetError, ThemeAssetState, ThemeHost } from '@nexus/theme-sdk';

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

// Host, von dem nur die Theme-ID zählt
function fakeHost(themeId: string): ThemeHost {
  return { colorScheme: { scheme: 'dark', themeId, mode: 'dark' } } as ThemeHost;
}

const wrapper = (themeId: string) => ({ children }: { children?: React.ReactNode }) => (
  <ThemeHostProvider value={fakeHost(themeId)}>{children}</ThemeHostProvider>
);

// Antwort des Proxys mit JSON-Fehler
const jsonError = (status: number, code: string, message: string) =>
  new Response(JSON.stringify({ code, message }), { status, headers: { 'Content-Type': 'application/json' } });

// Eine noch offene Anfrage, die sich per Signal abbrechen lässt
function pending(signalSink: AbortSignal[]) {
  return vi.fn((_url: string, init?: RequestInit) => {
    signalSink.push(init!.signal!);
    return new Promise<Response>((_, reject) => {
      init!.signal!.addEventListener('abort', () => reject(new DOMException('abgebrochen', 'AbortError')));
    });
  });
}

describe('themeAssetUrl', () => {
  it('baut die Proxy-URL mit kodierter Adresse', () => {
    expect(themeAssetUrl('minecraft', 'https://a.example.com/x.png')).toBe('/api/themes/minecraft/fetch?url=https%3A%2F%2Fa.example.com%2Fx.png');
  });

  it('hängt sha256 und entry kodiert an', () => {
    expect(themeAssetUrl('minecraft', 'https://a.example.com/c.jar', { sha256: 'ab12', entry: 'assets/minecraft/textures/block/dirt.png' })).toBe(
      '/api/themes/minecraft/fetch?url=https%3A%2F%2Fa.example.com%2Fc.jar&sha256=ab12&entry=assets%2Fminecraft%2Ftextures%2Fblock%2Fdirt.png',
    );
  });
});

describe('useThemeAsset', () => {
  let fetchMock: ReturnType<typeof vi.fn>;
  beforeEach(() => {
    fetchMock = vi.fn(async () => new Response('bytes', { status: 200 }));
    vi.stubGlobal('fetch', fetchMock);
  });

  it('meldet loading und danach ready mit der Proxy-URL des aktiven Themes', async () => {
    const { result } = renderHook(() => useThemeAsset('https://a.example.com/x.png'), { wrapper: wrapper('demo-theme') });
    expect(result.current).toEqual({ status: 'loading', url: null, error: null });
    const proxy = '/api/themes/demo-theme/fetch?url=https%3A%2F%2Fa.example.com%2Fx.png';
    await waitFor(() => expect(result.current).toEqual({ status: 'ready', url: proxy, error: null }));
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0][0]).toBe(proxy);
  });

  it('meldet den Fehlercode des Proxys mit HTTP-Status', async () => {
    fetchMock.mockResolvedValue(jsonError(403, 'theme_fetch_forbidden', 'Domain nicht freigegeben'));
    const { result } = renderHook(() => useThemeAsset('https://fremd.example.com/x.png'), { wrapper: wrapper('demo-theme') });
    await waitFor(() => expect(result.current.status).toBe('error'));
    expect(result.current.url).toBeNull();
    expect(result.current.error).toEqual<ThemeAssetError>({ status: 403, code: 'theme_fetch_forbidden', message: 'Domain nicht freigegeben' });
  });

  it('fällt ohne JSON-Body auf http_<status> zurück', async () => {
    fetchMock.mockResolvedValue(new Response('<html>', { status: 502 }));
    const { result } = renderHook(() => useThemeAsset('https://a.example.com/x.png'), { wrapper: wrapper('demo-theme') });
    await waitFor(() => expect(result.current.status).toBe('error'));
    expect(result.current.error?.status).toBe(502);
    expect(result.current.error?.code).toBe('http_502');
  });

  it('meldet einen Netzwerkfehler als code network', async () => {
    fetchMock.mockRejectedValue(new TypeError('Failed to fetch'));
    const { result } = renderHook(() => useThemeAsset('https://a.example.com/x.png'), { wrapper: wrapper('demo-theme') });
    await waitFor(() => expect(result.current.status).toBe('error'));
    expect(result.current.error).toMatchObject({ status: null, code: 'network' });
    expect(result.current.url).toBeNull();
  });

  it('nutzt opts.themeId statt der Host-ID und reicht sha256 und entry weiter', async () => {
    const { result } = renderHook(() => useThemeAsset('https://a.example.com/c.jar', { themeId: 'anderes', sha256: 'ff', entry: 'a/b.png' }), {
      wrapper: wrapper('demo-theme'),
    });
    const proxy = '/api/themes/anderes/fetch?url=https%3A%2F%2Fa.example.com%2Fc.jar&sha256=ff&entry=a%2Fb.png';
    await waitFor(() => expect(result.current.url).toBe(proxy));
    expect(fetchMock.mock.calls[0][0]).toBe(proxy);
  });

  it.each([[null], [undefined], ['']])('ist bei leerer URL (%s) idle und ruft kein fetch auf', (url) => {
    const { result } = renderHook(() => useThemeAsset(url), { wrapper: wrapper('demo-theme') });
    expect(result.current).toEqual({ status: 'idle', url: null, error: null });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('lädt bei URL-Wechsel neu und bricht die alte Anfrage ab', async () => {
    const signals: AbortSignal[] = [];
    fetchMock.mockImplementationOnce(pending(signals));
    const { result, rerender } = renderHook(({ url }) => useThemeAsset(url), {
      wrapper: wrapper('demo-theme'),
      initialProps: { url: 'https://a.example.com/1.png' },
    });
    expect(result.current.status).toBe('loading');
    rerender({ url: 'https://a.example.com/2.png' });
    expect(signals[0].aborted).toBe(true);
    expect(result.current.status).toBe('loading');
    const proxy = '/api/themes/demo-theme/fetch?url=https%3A%2F%2Fa.example.com%2F2.png';
    await waitFor(() => expect(result.current).toEqual({ status: 'ready', url: proxy, error: null }));
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('setzt nach dem Wechsel auf eine leere URL wieder idle und bricht ab', () => {
    const signals: AbortSignal[] = [];
    fetchMock.mockImplementationOnce(pending(signals));
    const { result, rerender } = renderHook(({ url }: { url: string | null }) => useThemeAsset(url), {
      wrapper: wrapper('demo-theme'),
      initialProps: { url: 'https://a.example.com/1.png' as string | null },
    });
    rerender({ url: null });
    expect(signals[0].aborted).toBe(true);
    expect(result.current).toEqual({ status: 'idle', url: null, error: null });
  });

  it('bricht beim Unmount die Anfrage ab', () => {
    const signals: AbortSignal[] = [];
    fetchMock.mockImplementationOnce(pending(signals));
    const { unmount } = renderHook(() => useThemeAsset('https://a.example.com/x.png'), { wrapper: wrapper('demo-theme') });
    unmount();
    expect(signals[0].aborted).toBe(true);
  });

  it('läuft mit opts.themeId auch ohne Host-Provider', async () => {
    const { result } = renderHook(() => useThemeAsset('https://a.example.com/x.png', { themeId: 'solo' }));
    await waitFor(() => expect(result.current.status).toBe('ready'));
    expect(result.current.url).toBe('/api/themes/solo/fetch?url=https%3A%2F%2Fa.example.com%2Fx.png');
  });

  it('wirft ohne Host-Provider und ohne opts.themeId einen klaren Fehler', () => {
    function Probe() {
      useThemeAsset('https://a.example.com/x.png');
      return null;
    }
    vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() => render(<Probe />)).toThrow(/useThemeAsset braucht den ThemeHostProvider des Nexus-Hosts oder opts\.themeId/);
    vi.mocked(console.error).mockRestore();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('lässt sich in einer Komponente als Bild mit Lade- und Fehleranzeige nutzen', async () => {
    function Bild() {
      const asset: ThemeAssetState = useThemeAsset('https://a.example.com/x.png');
      if (asset.status === 'loading') return <span>lädt</span>;
      if (asset.status === 'error') return <span>{asset.error!.code}</span>;
      return <img alt="x" src={asset.url ?? undefined} />;
    }
    render(<Bild />, { wrapper: wrapper('demo-theme') });
    expect(screen.getByText('lädt')).toBeTruthy();
    const img = await screen.findByRole('img');
    expect(img.getAttribute('src')).toBe('/api/themes/demo-theme/fetch?url=https%3A%2F%2Fa.example.com%2Fx.png');
  });
});
