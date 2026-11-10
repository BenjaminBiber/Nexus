// @vitest-environment jsdom
// Startanimation „Sunrise Drive“: Sonnenaufgang über dem Gitter, abbrechbar und abschaltbar
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { BOOT_KEY, PALETTE_KEY, SUNRISE_MS, SunriseDrive, rememberBoot, rememberPalette } from '../src/SunriseDrive.js';
import theme from '../src/main.js';

function mockMotion(reduce: boolean) {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: reduce && query.includes('prefers-reduced-motion'),
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  })) as unknown as typeof window.matchMedia;
}

beforeEach(() => {
  vi.useFakeTimers();
  mockMotion(false);
});

afterEach(() => {
  cleanup();
  window.localStorage.clear();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe('Startanimation „Sunrise Drive“', () => {
  it('ist als Slot StartAnimation angemeldet', () => {
    expect(theme.slots.StartAnimation).toBe(SunriseDrive);
  });

  it('zeigt die Szene und endet nach SUNRISE_MS genau einmal', () => {
    const onDone = vi.fn();
    render(<SunriseDrive onDone={onDone} />);
    const scene = screen.getByTestId('synth-sunrise');
    expect(scene.querySelector('.synth-sunrise-word')?.textContent).toBe('NEXUS');
    expect(scene.querySelector('.synth-sunrise-skip')?.textContent).toBe('PRESS START');
    act(() => vi.advanceTimersByTime(SUNRISE_MS - 1));
    expect(onDone).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(1));
    expect(onDone).toHaveBeenCalledTimes(1);
    act(() => vi.advanceTimersByTime(SUNRISE_MS));
    expect(onDone).toHaveBeenCalledTimes(1);
  });

  it('endet sofort bei Tastendruck, der Timer löst danach nicht erneut aus', () => {
    const onDone = vi.fn();
    render(<SunriseDrive onDone={onDone} />);
    fireEvent.keyDown(window, { key: 'Enter' });
    expect(onDone).toHaveBeenCalledTimes(1);
    act(() => vi.advanceTimersByTime(SUNRISE_MS));
    expect(onDone).toHaveBeenCalledTimes(1);
  });

  it('endet sofort bei Klick', () => {
    const onDone = vi.fn();
    render(<SunriseDrive onDone={onDone} />);
    fireEvent.pointerDown(window);
    expect(onDone).toHaveBeenCalledTimes(1);
    act(() => vi.advanceTimersByTime(SUNRISE_MS));
    expect(onDone).toHaveBeenCalledTimes(1);
  });

  it('endet ohne Markup sofort, wenn die Einstellung aus ist', () => {
    rememberBoot(false);
    expect(window.localStorage.getItem(BOOT_KEY)).toBe('off');
    const onDone = vi.fn();
    render(<SunriseDrive onDone={onDone} />);
    expect(onDone).toHaveBeenCalledTimes(1);
    expect(screen.queryByTestId('synth-sunrise')).toBeNull();
  });

  it('endet ohne Markup sofort bei reduzierter Bewegung', () => {
    mockMotion(true);
    const onDone = vi.fn();
    render(<SunriseDrive onDone={onDone} />);
    expect(onDone).toHaveBeenCalledTimes(1);
    expect(screen.queryByTestId('synth-sunrise')).toBeNull();
  });

  it('übernimmt das gemerkte Farbschema auf das eigene Wurzelelement', () => {
    rememberPalette('miami');
    expect(window.localStorage.getItem(PALETTE_KEY)).toBe('miami');
    render(<SunriseDrive onDone={() => undefined} />);
    expect(screen.getByTestId('synth-sunrise').getAttribute('data-synth-palette')).toBe('miami');
  });

  it('läuft auch mit gesperrtem localStorage ohne Fehler', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('gesperrt');
    });
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('gesperrt');
    });
    expect(() => rememberBoot(true)).not.toThrow();
    const onDone = vi.fn();
    render(<SunriseDrive onDone={onDone} />);
    expect(screen.getByTestId('synth-sunrise')).toBeTruthy();
    act(() => vi.advanceTimersByTime(SUNRISE_MS));
    expect(onDone).toHaveBeenCalledTimes(1);
  });

  it('entfernt beim Abbau Timer und Listener', () => {
    const onDone = vi.fn();
    const remove = vi.spyOn(window, 'removeEventListener');
    const { unmount } = render(<SunriseDrive onDone={onDone} />);
    unmount();
    expect(remove.mock.calls.map(([type]) => type)).toEqual(expect.arrayContaining(['keydown', 'pointerdown']));
    fireEvent.keyDown(window, { key: 'Enter' });
    act(() => vi.advanceTimersByTime(SUNRISE_MS));
    expect(onDone).not.toHaveBeenCalled();
  });
});
