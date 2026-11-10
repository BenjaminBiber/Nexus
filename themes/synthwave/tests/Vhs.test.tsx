// @vitest-environment jsdom
// VHS-Effekte: Bandzähler im Display und kurze Bildstörung beim Wechsel des Bereichs
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, renderHook } from '@testing-library/react';
import { formatCounter, TapeCounter } from '../src/TapeCounter.js';
import { TRACKING_MS, useTracking } from '../src/useTracking.js';

beforeEach(() => {
  vi.useFakeTimers();
  window.matchMedia = vi.fn().mockReturnValue({ matches: false }) as unknown as typeof window.matchMedia;
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  document.documentElement.removeAttribute('data-synth-tracking');
});

describe('formatCounter', () => {
  it('zeigt HH:MM:SS', () => {
    expect(formatCounter(0)).toBe('00:00:00');
    expect(formatCounter(872_000)).toBe('00:14:32');
    expect(formatCounter(3_600_000)).toBe('01:00:00');
  });

  it('hält über 99 Stunden bei 99:59:59 an', () => {
    expect(formatCounter(100 * 3_600_000)).toBe('99:59:59');
  });
});

describe('TapeCounter', () => {
  it('zählt im Sekundentakt und zeigt PLAY ▶ als <time> mit Uhrzeit im title', () => {
    const { container } = render(<TapeCounter vhs />);
    const time = container.querySelector('time.synth-osd');
    expect(time?.textContent).toContain('PLAY ▶');
    expect(time?.textContent).toContain('00:00:00');
    expect(time?.getAttribute('title')).toBeTruthy();
    expect(time?.getAttribute('dateTime')).toBeTruthy();
    act(() => vi.advanceTimersByTime(3000));
    expect(container.querySelector('time.synth-osd')?.textContent).toContain('00:00:03');
  });

  it('ohne VHS nur die Uhrzeit, kein PLAY', () => {
    const { container } = render(<TapeCounter vhs={false} />);
    expect(container.textContent).not.toContain('PLAY');
    expect(container.querySelector('.synth-clock')?.textContent).toMatch(/^\d\d:\d\d$/);
  });
});

describe('useTracking', () => {
  const root = () => document.documentElement.hasAttribute('data-synth-tracking');

  it('setzt beim ersten Rendern nichts, beim Wechsel für 160 ms das Attribut', () => {
    const { rerender } = renderHook(({ active }) => useTracking(active, true), { initialProps: { active: 'board' } });
    expect(root()).toBe(false);
    rerender({ active: 'questions' });
    expect(root()).toBe(true);
    act(() => vi.advanceTimersByTime(TRACKING_MS));
    expect(root()).toBe(false);
    expect(TRACKING_MS).toBe(160);
  });

  it('setzt bei VHS aus nichts', () => {
    const { rerender } = renderHook(({ active }) => useTracking(active, false), { initialProps: { active: 'board' } });
    rerender({ active: 'questions' });
    expect(root()).toBe(false);
  });

  it('setzt bei reduzierter Bewegung nichts', () => {
    window.matchMedia = vi.fn().mockReturnValue({ matches: true }) as unknown as typeof window.matchMedia;
    const { rerender } = renderHook(({ active }) => useTracking(active, true), { initialProps: { active: 'board' } });
    rerender({ active: 'questions' });
    expect(root()).toBe(false);
  });

  it('der Abbau räumt das Attribut auf', () => {
    const { rerender, unmount } = renderHook(({ active }) => useTracking(active, true), { initialProps: { active: 'board' } });
    rerender({ active: 'questions' });
    unmount();
    expect(root()).toBe(false);
  });
});
