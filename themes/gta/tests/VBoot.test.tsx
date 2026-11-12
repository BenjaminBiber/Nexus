// @vitest-environment jsdom
// Ladebildschirm im Stil GTA V: zwei Bildflächen, Wortmarke, Tipp, Lade-Kreis; endet nach Zeit, Klick oder Taste
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { BOOT_KEY } from '../src/settings.js';
import { BOOT_MS, TIPS, VBoot } from '../src/v/VBoot.js';

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

describe('VBoot', () => {
  it('zeigt zwei Bildflächen mit Szene, die Wortmarke, einen Tipp und „Lade Board“', () => {
    render(<VBoot onDone={vi.fn()} />);
    const screen = boot()!;
    expect(screen.classList.contains('gta-boot')).toBe(true);
    expect(screen.querySelectorAll('.gta-v-boot-panel svg')).toHaveLength(2);
    expect(screen.querySelector('[data-wordmark="nexus"]')).not.toBeNull();
    expect(screen.querySelector('.gta-v-boot-spin')?.textContent).toBe('Lade Board');
    expect(TIPS).toContain(screen.querySelector('.gta-v-boot-tip p')?.textContent);
  });

  it('hat mindestens sechs deutsche Tipps zum Board', () => {
    expect(TIPS.length).toBeGreaterThanOrEqual(6);
    expect(new Set(TIPS).size).toBe(TIPS.length);
    expect(TIPS).toContain('Jedes blockierte Ticket bringt dir einen Fahndungsstern ein.');
  });

  it('nimmt den Tipp zufällig', () => {
    vi.spyOn(Math, 'random').mockReturnValue(0.999);
    render(<VBoot onDone={vi.fn()} />);
    expect(boot()!.querySelector('.gta-v-boot-tip p')?.textContent).toBe(TIPS.at(-1));
  });

  it('endet nach BOOT_MS genau einmal', () => {
    const onDone = vi.fn();
    render(<VBoot onDone={onDone} />);
    act(() => vi.advanceTimersByTime(BOOT_MS - 1));
    expect(onDone).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(1));
    expect(onDone).toHaveBeenCalledTimes(1);
    act(() => vi.advanceTimersByTime(BOOT_MS));
    expect(onDone).toHaveBeenCalledTimes(1);
  });

  it('endet sofort bei Klick oder Taste', () => {
    const onDone = vi.fn();
    render(<VBoot onDone={onDone} />);
    fireEvent.pointerDown(window);
    expect(onDone).toHaveBeenCalledTimes(1);
    cleanup();
    const onKey = vi.fn();
    render(<VBoot onDone={onKey} />);
    fireEvent.keyDown(window, { key: 'Enter' });
    expect(onKey).toHaveBeenCalledTimes(1);
    act(() => vi.advanceTimersByTime(BOOT_MS));
    expect(onKey).toHaveBeenCalledTimes(1);
  });

  it('ist der Schalter Ladebildschirm aus, endet er sofort und zeigt nichts', () => {
    window.localStorage.setItem(BOOT_KEY, 'off');
    const onDone = vi.fn();
    const { container } = render(<VBoot onDone={onDone} />);
    expect(onDone).toHaveBeenCalledTimes(1);
    expect(container.innerHTML).toBe('');
  });

  it('bei reduzierter Bewegung endet er sofort und zeigt nichts', () => {
    mockMotion(true);
    const onDone = vi.fn();
    const { container } = render(<VBoot onDone={onDone} />);
    expect(onDone).toHaveBeenCalledTimes(1);
    expect(container.innerHTML).toBe('');
  });

  it('räumt Timer und Listener beim Abbauen weg', () => {
    const onDone = vi.fn();
    const { unmount } = render(<VBoot onDone={onDone} />);
    unmount();
    fireEvent.keyDown(window, { key: 'Enter' });
    act(() => vi.advanceTimersByTime(BOOT_MS));
    expect(onDone).not.toHaveBeenCalled();
  });
});
