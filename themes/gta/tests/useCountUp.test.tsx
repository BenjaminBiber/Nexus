// @vitest-environment jsdom
// Kontostand: zählt zwischen zwei geladenen Ständen desselben Boards hoch, springt beim ersten Stand und nach einem Wechsel
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { COUNT_MS, useCountUp } from '../src/shared/useCountUp.js';

beforeEach(() => {
  vi.useFakeTimers();
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({ matches: false, media: query })) as unknown as typeof window.matchMedia;
});
afterEach(() => vi.useRealTimers());

const mount = (target: number, live: boolean) =>
  renderHook(({ target, live }) => useCountUp(target, live), { initialProps: { target, live } });

describe('useCountUp', () => {
  it('zählt zwischen zwei geladenen Ständen hoch', () => {
    const { result, rerender } = mount(100, true);
    rerender({ target: 200, live: true });
    expect(result.current.counting).toBe(true);
    act(() => vi.advanceTimersByTime(COUNT_MS / 2));
    expect(result.current.value).toBeGreaterThan(100);
    expect(result.current.value).toBeLessThan(200);
    act(() => vi.advanceTimersByTime(COUNT_MS));
    expect(result.current).toEqual({ value: 200, counting: false });
  });

  it('der erste Stand springt, statt von 0 hochzuzählen', () => {
    const { result, rerender } = mount(0, false);
    rerender({ target: 87_500, live: true });
    expect(result.current).toEqual({ value: 87_500, counting: false });
    expect(vi.getTimerCount()).toBe(0);
  });

  it('der Sprung steht schon im ersten Render mit dem neuen Stand, kein Frame mit dem alten Wert', () => {
    const seen: { value: number; counting: boolean }[] = [];
    const { rerender } = renderHook(({ target, live }) => seen[seen.push(useCountUp(target, live)) - 1], { initialProps: { target: 0, live: false } });
    seen.length = 0;
    rerender({ target: 87_500, live: true });
    expect(seen[0]).toEqual({ value: 87_500, counting: false });
    expect(seen.every((s) => s.value === 87_500 && !s.counting)).toBe(true);
  });

  it('nach einem Board-Wechsel (Stand dazwischen weg) springt der neue Stand', () => {
    const { result, rerender } = mount(87_500, true);
    rerender({ target: 0, live: false });
    expect(result.current).toEqual({ value: 0, counting: false });
    rerender({ target: 40_000, live: true });
    expect(result.current).toEqual({ value: 40_000, counting: false });
    expect(vi.getTimerCount()).toBe(0);
  });

  it('ohne Angabe gilt der Wert als geladen', () => {
    const { result, rerender } = renderHook(({ target }) => useCountUp(target), { initialProps: { target: 1 } });
    rerender({ target: 2 });
    expect(result.current.counting).toBe(true);
  });
});
