// @vitest-environment jsdom
// Uhr: aktualisiert zur vollen Minute
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useClock } from '../src/shared/useClock.js';

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date(2026, 10, 11, 9, 41, 30));
});

afterEach(() => vi.useRealTimers());

describe('useClock', () => {
  it('liefert die Zeit und springt zur vollen Minute weiter', () => {
    const { result } = renderHook(() => useClock());
    expect(result.current.getMinutes()).toBe(41);
    act(() => vi.advanceTimersByTime(29_000));
    expect(result.current.getMinutes()).toBe(41);
    act(() => vi.advanceTimersByTime(1_000));
    expect(result.current.getMinutes()).toBe(42);
    act(() => vi.advanceTimersByTime(60_000));
    expect(result.current.getMinutes()).toBe(43);
  });

  it('räumt den Timer beim Abbauen weg', () => {
    const { unmount } = renderHook(() => useClock());
    unmount();
    expect(vi.getTimerCount()).toBe(0);
  });
});
