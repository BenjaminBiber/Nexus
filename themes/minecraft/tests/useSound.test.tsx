// @vitest-environment jsdom
// useSound: spielt die Töne nur bei eingeschaltetem Schalter „Sounds“; Audio gemockt, nie echter Ton
import { cleanup, renderHook } from '@testing-library/react';
import { ThemeHostProvider, type ThemeHost } from '@nexus/theme-sdk';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ResourcesProvider } from '../src/context.js';
import { useSound } from '../src/useSound.js';

const CLICK = '/api/themes/minecraft/fetch?url=click';

let created: Array<{ src: string; volume: number; currentTime: number; play: ReturnType<typeof vi.fn> }>;
let playResult: () => Promise<void>;

beforeEach(() => {
  created = [];
  playResult = () => Promise.resolve();
  vi.stubGlobal(
    'Audio',
    vi.fn(function (this: (typeof created)[number], src: string) {
      this.src = src;
      this.volume = 1;
      this.currentTime = 5;
      this.play = vi.fn(() => playResult());
      created.push(this);
    }),
  );
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

function wrapper(values: Record<string, boolean | string>, loading = false) {
  const host = { themeSettings: { values, loading, set: vi.fn() } } as unknown as ThemeHost;
  return ({ children }: { children?: ReactNode }) => (
    <ThemeHostProvider value={host}>
      <ResourcesProvider urls={{}} soundUrls={{ click: CLICK }}>
        {children}
      </ResourcesProvider>
    </ThemeHostProvider>
  );
}

describe('useSound', () => {
  it('sounds an: legt Audio mit der Proxy-URL an, spult zurück und spielt', () => {
    const { result } = renderHook(() => useSound(), { wrapper: wrapper({ sounds: true }) });
    result.current('click');
    expect(created).toHaveLength(1);
    expect(created[0].src).toBe(CLICK);
    expect(created[0].volume).toBe(0.5);
    expect(created[0].currentTime).toBe(0);
    expect(created[0].play).toHaveBeenCalledTimes(1);
  });

  it('legt je URL nur ein Audio an und benutzt es wieder', () => {
    const { result } = renderHook(() => useSound(), { wrapper: wrapper({ sounds: true }) });
    result.current('click');
    result.current('click');
    expect(created).toHaveLength(1);
    expect(created[0].play).toHaveBeenCalledTimes(2);
  });

  it('sounds aus oder Einstellungen laden noch → still', () => {
    renderHook(() => useSound(), { wrapper: wrapper({ sounds: false }) }).result.current('click');
    renderHook(() => useSound(), { wrapper: wrapper({ sounds: true }, true) }).result.current('click');
    expect(created).toHaveLength(0);
  });

  it('fehlende URL oder abgelehntes play() → kein Fehler', async () => {
    const { result } = renderHook(() => useSound(), { wrapper: wrapper({ sounds: true }) });
    expect(() => result.current('hotbar')).not.toThrow();
    expect(created).toHaveLength(0);
    playResult = () => Promise.reject(new Error('NotAllowedError'));
    expect(() => result.current('click')).not.toThrow();
    await Promise.resolve();
  });
});
