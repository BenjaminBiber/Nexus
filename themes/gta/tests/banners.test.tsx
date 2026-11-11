// @vitest-environment jsdom
// Banner nacheinander, je BANNER_MS lang; höchstens fünf warten; abgeschaltet nimmt nichts an
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { GameEvent } from '../src/data/events.js';
import { BANNER_MS, useBanners } from '../src/shared/banners.js';

const passed = (n: number): GameEvent => ({ kind: 'passed', ticketId: `N-${n}`, title: `Ticket ${n}`, reward: 40_000 });
const id = (event: GameEvent | null) => (event && event.kind !== 'wasted' ? event.ticketId : event?.kind ?? null);

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('useBanners', () => {
  it('zeigt Ereignisse nacheinander, jedes BANNER_MS lang', () => {
    expect(BANNER_MS).toBe(6000);
    const { result } = renderHook(() => useBanners(true));
    expect(result.current.current).toBeNull();
    act(() => result.current.push([passed(1), { kind: 'wasted' }]));
    expect(id(result.current.current)).toBe('N-1');
    act(() => vi.advanceTimersByTime(BANNER_MS - 1));
    expect(id(result.current.current)).toBe('N-1');
    act(() => vi.advanceTimersByTime(1));
    expect(id(result.current.current)).toBe('wasted');
    act(() => vi.advanceTimersByTime(BANNER_MS));
    expect(result.current.current).toBeNull();
  });

  it('ein neues Ereignis verlängert das laufende Banner nicht', () => {
    const { result } = renderHook(() => useBanners(true));
    act(() => result.current.push([passed(1)]));
    act(() => vi.advanceTimersByTime(4000));
    act(() => result.current.push([passed(2)]));
    act(() => vi.advanceTimersByTime(2000));
    expect(id(result.current.current)).toBe('N-2');
  });

  it('dismiss springt zum nächsten', () => {
    const { result } = renderHook(() => useBanners(true));
    act(() => result.current.push([passed(1), passed(2)]));
    act(() => result.current.dismiss());
    expect(id(result.current.current)).toBe('N-2');
    act(() => vi.advanceTimersByTime(BANNER_MS - 1));
    expect(id(result.current.current)).toBe('N-2');
    act(() => result.current.dismiss());
    expect(result.current.current).toBeNull();
    act(() => result.current.dismiss());
    expect(result.current.current).toBeNull();
  });

  it('höchstens fünf warten, ältere fallen weg', () => {
    const { result } = renderHook(() => useBanners(true));
    act(() => result.current.push([1, 2, 3, 4, 5, 6, 7, 8].map(passed)));
    const seen = [id(result.current.current)];
    for (let i = 0; i < 6; i++) {
      act(() => result.current.dismiss());
      seen.push(id(result.current.current));
    }
    expect(seen).toEqual(['N-1', 'N-4', 'N-5', 'N-6', 'N-7', 'N-8', null]);
  });

  it('abgeschaltet: push nimmt nichts an, current ist null', () => {
    const { result, rerender } = renderHook(({ on }) => useBanners(on), { initialProps: { on: false } });
    act(() => result.current.push([passed(1)]));
    expect(result.current.current).toBeNull();
    rerender({ on: true });
    expect(result.current.current).toBeNull();
    act(() => result.current.push([passed(2)]));
    expect(id(result.current.current)).toBe('N-2');
    rerender({ on: false });
    expect(result.current.current).toBeNull();
    rerender({ on: true });
    expect(result.current.current).toBeNull();
  });

  it('push und dismiss bleiben stabil; beim Abbauen keine Timer', () => {
    const { result, rerender, unmount } = renderHook(() => useBanners(true));
    const { push, dismiss } = result.current;
    act(() => push([passed(1)]));
    rerender();
    expect(result.current.push).toBe(push);
    expect(result.current.dismiss).toBe(dismiss);
    unmount();
    expect(vi.getTimerCount()).toBe(0);
  });
});
