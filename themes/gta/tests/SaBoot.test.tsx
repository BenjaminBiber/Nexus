// @vitest-environment jsdom
// Ladebildschirm im Stil San Andreas: Szene, Wortmarke, Tipp und Ladebalken; endet nach Zeit, Klick oder Taste
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { SaBoot, TIPS } from '../src/sa/SaBoot.js';
import { BOOT_KEY } from '../src/settings.js';
import { BOOT_MS } from '../src/shared/useBoot.js';
import { TIPS as V_TIPS } from '../src/v/VBoot.js';

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

const boot = () => document.querySelector('[data-gta-part="boot"]') as HTMLElement | null;

describe('SaBoot', () => {
  it('zeigt die Szene, die Wortmarke im gotischen Look, einen Tipp und den Ladebalken', () => {
    render(<SaBoot onDone={vi.fn()} />);
    const screen = boot()!;
    expect(screen.classList.contains('gta-boot')).toBe(true);
    expect(screen.classList.contains('gta-sa-boot')).toBe(true);
    expect(screen.querySelector('svg.gta-sa-boot-scene')).not.toBeNull();
    expect(screen.querySelector('[data-wordmark="nexus"]')).not.toBeNull();
    expect(screen.querySelector('.gta-sa-boot-bar')).not.toBeNull();
    expect(TIPS).toContain(screen.querySelector('.gta-sa-boot-tip')?.textContent);
  });

  it('hat mindestens sechs eigene deutsche Tipps im Ton von San Andreas', () => {
    expect(TIPS.length).toBeGreaterThanOrEqual(6);
    expect(new Set(TIPS).size).toBe(TIPS.length);
    expect(TIPS).toContain('Erledigte Aufträge bringen Respekt. Respekt bringt Fortschritt.');
    for (const tip of TIPS) expect(V_TIPS).not.toContain(tip);
  });

  it('nimmt den Tipp zufällig', () => {
    vi.spyOn(Math, 'random').mockReturnValue(0.999);
    render(<SaBoot onDone={vi.fn()} />);
    expect(boot()!.querySelector('.gta-sa-boot-tip')?.textContent).toBe(TIPS.at(-1));
  });

  it('endet nach BOOT_MS genau einmal', () => {
    const onDone = vi.fn();
    render(<SaBoot onDone={onDone} />);
    act(() => vi.advanceTimersByTime(BOOT_MS - 1));
    expect(onDone).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(1));
    expect(onDone).toHaveBeenCalledTimes(1);
    act(() => vi.advanceTimersByTime(BOOT_MS));
    expect(onDone).toHaveBeenCalledTimes(1);
  });

  it('endet sofort bei Klick oder Taste', () => {
    const onDone = vi.fn();
    render(<SaBoot onDone={onDone} />);
    fireEvent.pointerDown(window);
    expect(onDone).toHaveBeenCalledTimes(1);
    cleanup();
    const onKey = vi.fn();
    render(<SaBoot onDone={onKey} />);
    fireEvent.keyDown(window, { key: 'Enter' });
    expect(onKey).toHaveBeenCalledTimes(1);
  });

  it('ist der Schalter Ladebildschirm aus, endet er sofort und zeigt nichts', () => {
    window.localStorage.setItem(BOOT_KEY, 'off');
    const onDone = vi.fn();
    const { container } = render(<SaBoot onDone={onDone} />);
    expect(onDone).toHaveBeenCalledTimes(1);
    expect(container.innerHTML).toBe('');
  });

  it('räumt Timer und Listener beim Abbauen weg', () => {
    const onDone = vi.fn();
    const { unmount } = render(<SaBoot onDone={onDone} />);
    unmount();
    fireEvent.keyDown(window, { key: 'Enter' });
    act(() => vi.advanceTimersByTime(BOOT_MS));
    expect(onDone).not.toHaveBeenCalled();
  });
});
