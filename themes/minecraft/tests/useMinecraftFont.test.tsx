// @vitest-environment jsdom
// Spielschrift beim Start: aus dem Cache holen oder bauen, in der Cache API ablegen und als FontFace registrieren.
// Alle Browser-APIs sind gestubbt (caches, FontFace, document.fonts, Image, Canvas); `buildFont` wird gezählt.
import { renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const built = vi.hoisted(() => ({ calls: 0 }));
vi.mock('../src/font/buildFont.js', async (orig) => ({
  ...(await orig<typeof import('../src/font/buildFont.js')>()),
  buildFont: () => {
    built.calls++;
    return new ArrayBuffer(8);
  },
}));
const { useMinecraftFont } = await import('../src/font/useMinecraftFont.js');
const { FONT_ATLAS, FONT_ACCENTED, FONT_NONLATIN, FONT_DEFAULT_JSON, FONT_SPACE_JSON } = await import('../src/resources.js');

const urls = {
  [FONT_ATLAS]: '/a',
  [FONT_ACCENTED]: '/b',
  [FONT_NONLATIN]: '/c',
  [FONT_DEFAULT_JSON]: '/d.json',
  [FONT_SPACE_JSON]: '/s.json',
};
let store: Map<string, ArrayBuffer>;
let added: string[];
let deleted: number;

beforeEach(() => {
  built.calls = 0;
  added = [];
  deleted = 0;
  store = new Map();
  vi.stubGlobal('caches', {
    open: async () => ({
      match: async (k: string) => (store.has(k) ? new Response(store.get(k)) : undefined),
      put: async (k: string, r: Response) => {
        store.set(k, await r.arrayBuffer());
      },
    }),
  });
  vi.stubGlobal(
    'FontFace',
    class {
      family: string;
      constructor(f: string) {
        this.family = f;
      }
      load() {
        return Promise.resolve(this);
      }
    },
  );
  Object.defineProperty(document, 'fonts', {
    configurable: true,
    value: {
      add: (f: { family: string }) => added.push(f.family),
      delete: () => {
        deleted++;
        return true;
      },
    },
  });
  vi.stubGlobal(
    'fetch',
    vi.fn(
      async (u: string) =>
        new Response(
          JSON.stringify(
            u.endsWith('s.json')
              ? { providers: [{ type: 'space', advances: { ' ': 4 } }] }
              : { providers: [{ type: 'bitmap', file: 'minecraft:font/ascii.png', ascent: 7, chars: ['A'] }] },
          ),
        ),
    ),
  );
  // Image + Canvas: ein 8×8-Atlas mit einem gesetzten Pixel
  vi.stubGlobal(
    'Image',
    class {
      onload: (() => void) | null = null;
      naturalWidth = 8;
      naturalHeight = 8;
      set src(_: string) {
        queueMicrotask(() => this.onload?.());
      }
    },
  );
  HTMLCanvasElement.prototype.getContext = vi.fn(() => ({
    drawImage() {},
    getImageData: () => ({ width: 8, height: 8, data: new Uint8ClampedArray(8 * 8 * 4).fill(255) }),
  })) as never;
});
afterEach(() => vi.unstubAllGlobals());

describe('useMinecraftFont', () => {
  it('baut ohne Cache-Treffer, legt ab und registriert die Schrift', async () => {
    const { result, unmount } = renderHook(() => useMinecraftFont(urls));
    await waitFor(() => expect(result.current).toBe('ready'));
    expect(built.calls).toBe(1);
    expect([...store.keys()][0]).toMatch(/^\/font\/1\.21\.4\//);
    expect(added).toEqual(['Minecraft']);
    unmount();
    expect(deleted).toBe(1);
  });

  it('nimmt die Schrift aus dem Cache, ohne zu bauen', async () => {
    const { result: first } = renderHook(() => useMinecraftFont(urls));
    await waitFor(() => expect(first.current).toBe('ready'));
    built.calls = 0;
    const { result } = renderHook(() => useMinecraftFont(urls));
    await waitFor(() => expect(result.current).toBe('ready'));
    expect(built.calls).toBe(0);
  });

  it('bleibt idle, solange Atlanten fehlen', () => {
    const { result } = renderHook(() => useMinecraftFont({}));
    expect(result.current).toBe('idle');
  });
});
