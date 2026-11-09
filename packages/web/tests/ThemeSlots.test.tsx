// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import { registerTheme, unregisterTheme } from '../src/theme/registry.js';
import { SlotBoundary } from '../src/theme/SlotBoundary.js';
import { useSlot } from '../src/theme/useSlot.js';
import { THEME_EVENT } from '../src/theme/theme.js';

afterEach(() => {
  cleanup();
  unregisterTheme('test-slots');
  delete document.documentElement.dataset.theme;
});

// Zeigt den Namen der Progress-Komponente des angezeigten Themes oder „standard“
function ProgressProbe() {
  const Progress = useSlot('Progress');
  return <span>{Progress ? 'eigen' : 'standard'}</span>;
}

// Wirft beim Rendern (für die Error Boundary)
function Broken(): never {
  throw new Error('kaputtes Theme');
}

describe('useSlot', () => {
  it('liefert ohne registriertes Theme nichts', () => {
    render(<ProgressProbe />);
    expect(screen.getByText('standard')).toBeTruthy();
  });

  it('liefert den Slot des angezeigten Themes und folgt einem Wechsel', () => {
    registerTheme({ id: 'test-slots', slots: { Progress: () => <span>x</span> } });
    render(<ProgressProbe />);
    expect(screen.getByText('standard')).toBeTruthy();
    act(() => {
      document.documentElement.dataset.theme = 'test-slots';
      window.dispatchEvent(new CustomEvent(THEME_EVENT));
    });
    expect(screen.getByText('eigen')).toBeTruthy();
  });
});

describe('SlotBoundary', () => {
  it('zeigt bei einem Fehler der Theme-Komponente den Rückfall', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    render(
      <SlotBoundary fallback={<span>rueckfall</span>} resetKey="test-slots">
        <Broken />
      </SlotBoundary>,
    );
    expect(screen.getByText('rueckfall')).toBeTruthy();
    spy.mockRestore();
  });

  it('versucht es nach einem Theme-Wechsel erneut', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const { rerender } = render(
      <SlotBoundary fallback={<span>rueckfall</span>} resetKey="a">
        <Broken />
      </SlotBoundary>,
    );
    rerender(
      <SlotBoundary fallback={<span>rueckfall</span>} resetKey="b">
        <span>wieder da</span>
      </SlotBoundary>,
    );
    expect(screen.getByText('wieder da')).toBeTruthy();
    spy.mockRestore();
  });
});
