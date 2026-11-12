// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { MemoryRouter } from 'react-router-dom';
import type { StartAnimationProps } from '@nexus/theme-sdk';
import { registerTheme, unregisterTheme } from '../src/theme/registry.js';
import { THEME_EVENT } from '../src/theme/theme.js';
import { START_ANIMATION_MAX_MS, StartAnimation } from '../src/components/StartAnimation.js';
import { App } from '../src/App.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import { fakeApi } from './helpers/fakeApi.js';

// Stellt „prefers-reduced-motion“ ein
function stubMotion(reduced: boolean) {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches: reduced && query.includes('reduced-motion'),
    media: query,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
  }));
}

beforeEach(() => {
  const overlay = document.createElement('div');
  overlay.id = 'start-anim';
  document.body.appendChild(overlay);
  document.documentElement.dataset.theme = 'test-start';
  stubMotion(false);
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  unregisterTheme('test-start');
  unregisterTheme('test-other');
  document.getElementById('start-anim')?.remove();
  delete document.documentElement.dataset.theme;
});

// Ladebildschirm des Test-Themes mit Knopf zum Beenden
function ThemeStart({ onDone }: StartAnimationProps) {
  return (
    <div data-testid="theme-start">
      <button type="button" onClick={onDone}>fertig</button>
    </div>
  );
}

// Wirft beim Rendern
function Broken(): never {
  throw new Error('kaputte Startanimation');
}

describe('StartAnimation', () => {
  it('rendert ohne Slot nichts und lässt das Standard-Overlay stehen', () => {
    render(<StartAnimation />);
    expect(screen.queryByTestId('theme-start')).toBeNull();
    expect(document.getElementById('start-anim')).not.toBeNull();
  });

  it('zeigt die Komponente des Themes statt des Overlays bis onDone', () => {
    registerTheme({ id: 'test-start', slots: { StartAnimation: ThemeStart } });
    render(<StartAnimation />);
    expect(screen.getByTestId('theme-start')).toBeTruthy();
    expect(document.getElementById('start-anim')).toBeNull();
    fireEvent.click(screen.getByText('fertig'));
    expect(screen.queryByTestId('theme-start')).toBeNull();
  });

  it('endet spätestens nach 8 s, wenn onDone nie kommt', () => {
    vi.useFakeTimers();
    registerTheme({ id: 'test-start', slots: { StartAnimation: () => <div data-testid="theme-start" /> } });
    render(<StartAnimation />);
    expect(START_ANIMATION_MAX_MS).toBe(8000);
    act(() => vi.advanceTimersByTime(START_ANIMATION_MAX_MS - 1));
    expect(screen.getByTestId('theme-start')).toBeTruthy();
    act(() => vi.advanceTimersByTime(1));
    expect(screen.queryByTestId('theme-start')).toBeNull();
  });

  it('lässt das Standard-Overlay laufen, wenn die Komponente beim Rendern wirft', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    registerTheme({ id: 'test-start', slots: { StartAnimation: Broken } });
    render(<StartAnimation />);
    expect(document.getElementById('start-anim')).not.toBeNull();
    expect(document.querySelector('.theme-start-anim')).toBeNull();
    expect(spy).toHaveBeenCalled();
  });

  it('zeigt sofort die App, wenn die Komponente nach der Übernahme wirft', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    // Wirft erst nach einem Klick
    function LaterBroken() {
      const [broken, setBroken] = useBroken();
      if (broken) throw new Error('später kaputt');
      return <button type="button" onClick={() => setBroken(true)}>kaputt</button>;
    }
    registerTheme({ id: 'test-start', slots: { StartAnimation: LaterBroken } });
    render(<StartAnimation />);
    expect(document.getElementById('start-anim')).toBeNull();
    fireEvent.click(screen.getByText('kaputt'));
    expect(document.querySelector('.theme-start-anim')).toBeNull();
    expect(spy).toHaveBeenCalled();
  });

  it('zeigt bei prefers-reduced-motion keine Startanimation', () => {
    stubMotion(true);
    registerTheme({ id: 'test-start', slots: { StartAnimation: ThemeStart } });
    render(<StartAnimation />);
    expect(screen.queryByTestId('theme-start')).toBeNull();
  });

  it('zeigt bei einem späteren Theme-Wechsel keine Startanimation', () => {
    registerTheme({ id: 'test-other', slots: { StartAnimation: ThemeStart } });
    render(<StartAnimation />);
    act(() => {
      document.documentElement.dataset.theme = 'test-other';
      window.dispatchEvent(new CustomEvent(THEME_EVENT));
    });
    expect(screen.queryByTestId('theme-start')).toBeNull();
  });

  it('zeigt in der App die Startanimation über der geladenen App', async () => {
    registerTheme({ id: 'test-start', slots: { StartAnimation: ThemeStart } });
    const settings = { notifications: 'off', heroImages: false, claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false };
    render(
      <ApiProvider api={fakeApi({ getUserSettings: async () => ({ ...settings, theme: 'test-start' }) })}>
        <MemoryRouter initialEntries={['/p/p1/sessions']}>
          <App />
        </MemoryRouter>
      </ApiProvider>,
    );
    expect(screen.getByTestId('theme-start')).toBeTruthy();
    expect(await screen.findByRole('navigation', { name: 'Hauptnavigation' })).toBeTruthy();
    fireEvent.click(screen.getByText('fertig'));
    expect(screen.queryByTestId('theme-start')).toBeNull();
  });
});

// Zustand „kaputt“ für LaterBroken
function useBroken() {
  return useState(false);
}
