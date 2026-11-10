// @vitest-environment jsdom
// Slots Progress, StartAnimation und Mermaid des Terminal-Themes
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { BOOT_HOLD_MS, BOOT_KEY, BOOT_STEP_MS, BootScreen, LAST_LOGIN_KEY, rememberBoot } from '../src/BootScreen.js';
import { terminalMermaid } from '../src/mermaid.js';
import { TerminalProgress } from '../src/TerminalProgress.js';

afterEach(() => {
  cleanup();
  window.localStorage.clear();
  vi.useRealTimers();
});

describe('Slot Progress', () => {
  it('zeigt einen Balken aus Zeichen mit Zähler und ist ein Fortschrittsbalken', () => {
    render(<TerminalProgress done={5} total={10} label="Fortschritt der Schritte" caption="Schritte erledigt" />);
    const bar = screen.getByRole('progressbar', { name: 'Fortschritt der Schritte' });
    expect(bar.getAttribute('aria-valuenow')).toBe('5');
    expect(bar.getAttribute('aria-valuemax')).toBe('10');
    expect(bar.textContent).toBe('[#####·····]5/10Schritte erledigt');
  });

  it('bleibt bei 0 von 0 leer und läuft nie über', () => {
    const { rerender } = render(<TerminalProgress done={0} total={0} label="p" caption="" />);
    expect(screen.getByRole('progressbar').textContent).toContain('[··········]');
    rerender(<TerminalProgress done={12} total={10} label="p" caption="" />);
    expect(screen.getByRole('progressbar').textContent).toContain('[##########]');
  });
});

describe('Slot StartAnimation', () => {
  beforeEach(() => vi.useFakeTimers());

  it('zeigt Boot-Meldungen und endet nach dem letzten Schritt von selbst', () => {
    const onDone = vi.fn();
    render(<BootScreen onDone={onDone} />);
    expect(screen.getByTestId('term-boot').textContent).toContain('_ __');
    act(() => vi.advanceTimersByTime(BOOT_STEP_MS * 2));
    expect(screen.getAllByText(/Started/)).toHaveLength(2);
    act(() => vi.advanceTimersByTime(BOOT_STEP_MS * 4));
    expect(screen.getByText(/Willkommen bei Nexus/)).toBeTruthy();
    expect(onDone).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(BOOT_HOLD_MS));
    expect(onDone).toHaveBeenCalledTimes(1);
  });

  it('endet sofort bei einer Taste, nur einmal', () => {
    const onDone = vi.fn();
    render(<BootScreen onDone={onDone} />);
    fireEvent.keyDown(window, { key: 'x' });
    fireEvent.pointerDown(window);
    act(() => vi.advanceTimersByTime(5000));
    expect(onDone).toHaveBeenCalledTimes(1);
  });

  it('zeigt beim nächsten Start den letzten Login', () => {
    window.localStorage.setItem(LAST_LOGIN_KEY, '2026-11-09T08:15:00.000Z');
    render(<BootScreen onDone={() => undefined} />);
    act(() => vi.advanceTimersByTime(BOOT_STEP_MS * 6));
    expect(screen.getByText(/Last login: .* on tty1/)).toBeTruthy();
    expect(window.localStorage.getItem(LAST_LOGIN_KEY)).not.toBe('2026-11-09T08:15:00.000Z');
  });

  it('läuft nicht, wenn die Einstellung sie ausschaltet', () => {
    rememberBoot(false);
    expect(window.localStorage.getItem(BOOT_KEY)).toBe('off');
    const onDone = vi.fn();
    render(<BootScreen onDone={onDone} />);
    expect(onDone).toHaveBeenCalledTimes(1);
    expect(screen.queryByTestId('term-boot')).toBeNull();
  });
});

describe('Mermaid', () => {
  it('nimmt das Schema „base“ mit den Farben und der Schrift des Themes', () => {
    document.documentElement.style.setProperty('--accent', '#bd93f9');
    document.documentElement.style.setProperty('--font-mono', "'JetBrains Mono', monospace");
    const look = terminalMermaid();
    expect(look.theme).toBe('base');
    expect(look.fontFamily).toBe("'JetBrains Mono', monospace");
    expect(look.themeVariables?.primaryBorderColor).toBe('#bd93f9');
    expect(look.themeCSS).toContain('rx: 0');
    document.documentElement.removeAttribute('style');
  });
});
