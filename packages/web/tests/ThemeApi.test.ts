import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiClient } from '../src/api/ApiClient.js';
import { ApiError } from '../src/api/ApiError.js';

afterEach(() => vi.unstubAllGlobals());

type Call = { url: string; method: string; body?: string };

// Ersetzt fetch und schreibt mit, was ankam
function stubFetch(status: number, body: string | null): Call[] {
  const calls: Call[] = [];
  vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
    calls.push({ url, method: init.method ?? 'GET', body: init.body as string | undefined });
    return new Response(body, { status });
  }));
  return calls;
}

describe('ApiClient für Themes', () => {
  it('listet die installierten Themes', async () => {
    const calls = stubFetch(200, '[{"id":"nacht"}]');
    const themes = await new ApiClient().listThemes();
    expect(themes).toEqual([{ id: 'nacht' }]);
    expect(calls).toEqual([{ url: '/api/themes', method: 'GET', body: undefined }]);
  });

  it('lädt ein Paket als Base64 hoch', async () => {
    const calls = stubFetch(200, '{"id":"nacht"}');
    const bytes = new Uint8Array([0x50, 0x4b, 0x03, 0x04, 0xff, 0x00]);
    const manifest = await new ApiClient().installTheme(new Blob([bytes]));
    expect(manifest).toEqual({ id: 'nacht' });
    expect(calls[0].url).toBe('/api/themes/install');
    expect(calls[0].method).toBe('POST');
    expect(JSON.parse(calls[0].body!)).toEqual({ data: 'UEsDBP8A' });

    await new ApiClient().installTheme(bytes.buffer);
    expect(JSON.parse(calls[1].body!)).toEqual({ data: 'UEsDBP8A' });
  });

  it('kodiert auch große Pakete ohne Stapelüberlauf', async () => {
    const calls = stubFetch(200, '{}');
    const bytes = new Uint8Array(3 * 1024 * 1024).fill(65);
    await new ApiClient().installTheme(bytes.buffer);
    const data = JSON.parse(calls[0].body!).data as string;
    expect(data.length).toBe(4 * 1024 * 1024);
    expect(data.startsWith('QUFB')).toBe(true);
  });

  it('entfernt ein Theme mit kodierter id', async () => {
    const calls = stubFetch(204, null);
    await new ApiClient().removeTheme('a/b');
    expect(calls).toEqual([{ url: '/api/themes/a%2Fb', method: 'DELETE', body: undefined }]);
  });

  it('übersetzt 422 mit Feldfehlern in ApiError', async () => {
    stubFetch(422, JSON.stringify({
      code: 'theme-manifest-invalid',
      message: 'Das Manifest ist ungültig',
      details: { errors: [{ field: 'id', message: 'falsch' }] },
    }));
    const error = await new ApiClient().installTheme(new Blob([])).catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect((error as ApiError).code).toBe('theme-manifest-invalid');
    expect((error as ApiError).status).toBe(422);
    expect((error as ApiError).details).toEqual({ errors: [{ field: 'id', message: 'falsch' }] });
  });
});
