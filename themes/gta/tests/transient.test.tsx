// @vitest-environment jsdom
// Kurz eingeblendete Werte: nach einer Änderung bzw. wenn ein Zähler steigt
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useChanged, useIncrease } from '../src/shared/transient.js';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('useChanged', () => {
  it('der erste definierte Wert löst nichts aus, eine Änderung für ms Millisekunden', () => {
    const { result, rerender } = renderHook(({ value }) => useChanged(value, 2500), { initialProps: { value: undefined as string | undefined | null } });
    expect(result.current).toBeNull();
    rerender({ value: 'Board' });
    expect(result.current).toBeNull();
    rerender({ value: 'Board' });
    expect(result.current).toBeNull();
    rerender({ value: 'Fragen' });
    expect(result.current).toBe('Fragen');
    act(() => vi.advanceTimersByTime(2499));
    expect(result.current).toBe('Fragen');
    act(() => vi.advanceTimersByTime(1));
    expect(result.current).toBeNull();
  });

  it('eine weitere Änderung startet die Zeit neu', () => {
    const { result, rerender } = renderHook(({ value }) => useChanged(value, 2500), { initialProps: { value: 'A' } });
    rerender({ value: 'B' });
    act(() => vi.advanceTimersByTime(2000));
    rerender({ value: 'C' });
    act(() => vi.advanceTimersByTime(2000));
    expect(result.current).toBe('C');
    act(() => vi.advanceTimersByTime(500));
    expect(result.current).toBeNull();
  });

  it('null dazwischen zählt nicht als Änderung', () => {
    const { result, rerender } = renderHook(({ value }) => useChanged(value, 2500), { initialProps: { value: 'A' as string | null } });
    rerender({ value: null });
    expect(result.current).toBeNull();
    rerender({ value: 'A' });
    expect(result.current).toBeNull();
    rerender({ value: null });
    rerender({ value: 'B' });
    expect(result.current).toBe('B');
  });

  it('räumt den Timer beim Abbauen weg', () => {
    const { rerender, unmount } = renderHook(({ value }) => useChanged(value, 2500), { initialProps: { value: 'A' } });
    rerender({ value: 'B' });
    unmount();
    expect(vi.getTimerCount()).toBe(0);
  });
});

describe('useIncrease', () => {
  it('der erste Wert löst nichts aus; ein Anstieg meldet die Zahl der neuen', () => {
    const { result, rerender } = renderHook(({ count }) => useIncrease(count, 15_000, true), { initialProps: { count: 3 } });
    expect(result.current.value).toBeNull();
    rerender({ count: 2 });
    expect(result.current.value).toBeNull();
    rerender({ count: 4 });
    expect(result.current.value).toBe(2);
  });

  it('weitere Anstiege zählen dazu und starten die Zeit neu; nach ms weg', () => {
    const { result, rerender } = renderHook(({ count }) => useIncrease(count, 15_000, true), { initialProps: { count: 0 } });
    rerender({ count: 1 });
    act(() => vi.advanceTimersByTime(10_000));
    rerender({ count: 3 });
    expect(result.current.value).toBe(3);
    act(() => vi.advanceTimersByTime(14_999));
    expect(result.current.value).toBe(3);
    act(() => vi.advanceTimersByTime(1));
    expect(result.current.value).toBeNull();
  });

  it('clear() blendet sofort aus', () => {
    const { result, rerender } = renderHook(({ count }) => useIncrease(count, 15_000, true), { initialProps: { count: 0 } });
    rerender({ count: 1 });
    act(() => result.current.clear());
    expect(result.current.value).toBeNull();
    expect(vi.getTimerCount()).toBe(0);
    rerender({ count: 2 });
    expect(result.current.value).toBe(1);
  });

  it('abgeschaltet meldet es nichts und holt Anstiege auch später nicht nach', () => {
    const { result, rerender } = renderHook(({ count, on }) => useIncrease(count, 15_000, on), { initialProps: { count: 0, on: false } });
    rerender({ count: 2, on: false });
    expect(result.current.value).toBeNull();
    rerender({ count: 2, on: true });
    expect(result.current.value).toBeNull();
    rerender({ count: 3, on: true });
    expect(result.current.value).toBe(1);
    rerender({ count: 3, on: false });
    expect(result.current.value).toBeNull();
  });
});
