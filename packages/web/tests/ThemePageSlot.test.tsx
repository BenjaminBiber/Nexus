// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import type { PageProps } from '@nexus/theme-sdk';
import { registerTheme, unregisterTheme } from '../src/theme/registry.js';
import { THEME_EVENT } from '../src/theme/theme.js';
import { ThemePage } from '../src/theme/ThemePage.js';

afterEach(() => {
  cleanup();
  unregisterTheme('test-page');
  unregisterTheme('test-page-2');
  delete document.documentElement.dataset.theme;
});

// Setzt das angezeigte Theme und meldet den Wechsel
function showTheme(id: string) {
  document.documentElement.dataset.theme = id;
  window.dispatchEvent(new CustomEvent(THEME_EVENT));
}

// Theme-Seite, die ihre Projekt-ID zeigt
function ThemeDashboard({ pid }: PageProps) {
  return <span>theme-dashboard {pid}</span>;
}

// Wirft beim Rendern (für die Error Boundary)
function Broken(): never {
  throw new Error('kaputte Seite');
}

// Routen wie in der App: Dashboard über ThemePage, Inbox immer Standard
function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path="/p/:pid">
          <Route path="dashboard" element={<ThemePage name="dashboard" standard={<span>standard-dashboard</span>} />} />
          <Route path="inbox" element={<span>standard-inbox</span>} />
        </Route>
      </Routes>
    </MemoryRouter>,
  );
}

describe('Seiten-Slot page:dashboard', () => {
  it('ersetzt das Dashboard durch die Theme-Seite und reicht pid weiter', () => {
    registerTheme({ id: 'test-page', slots: { 'page:dashboard': ThemeDashboard } });
    showTheme('test-page');
    renderAt('/p/x/dashboard');
    expect(screen.getByText('theme-dashboard x')).toBeTruthy();
    expect(screen.queryByText('standard-dashboard')).toBeNull();
  });

  it('lässt andere Seiten Standard', () => {
    registerTheme({ id: 'test-page', slots: { 'page:dashboard': ThemeDashboard } });
    showTheme('test-page');
    renderAt('/p/x/inbox');
    expect(screen.getByText('standard-inbox')).toBeTruthy();
  });

  it('zeigt ohne Page-Slot das Standard-Dashboard', () => {
    registerTheme({ id: 'test-page', slots: {} });
    showTheme('test-page');
    renderAt('/p/x/dashboard');
    expect(screen.getByText('standard-dashboard')).toBeTruthy();
  });

  it('fällt bei einem Fehler der Theme-Seite auf das Standard-Dashboard zurück und versucht nach Theme-Wechsel erneut', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    registerTheme({ id: 'test-page', slots: { 'page:dashboard': Broken } });
    registerTheme({ id: 'test-page-2', slots: { 'page:dashboard': ThemeDashboard } });
    showTheme('test-page');
    renderAt('/p/x/dashboard');
    expect(screen.getByText('standard-dashboard')).toBeTruthy();
    expect(spy).toHaveBeenCalled();
    act(() => showTheme('test-page-2'));
    expect(screen.getByText('theme-dashboard x')).toBeTruthy();
    spy.mockRestore();
  });
});
