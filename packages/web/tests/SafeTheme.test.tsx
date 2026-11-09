// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { loadActiveThemeModule } from '../src/theme/loadThemeModule.js';
import { isSafeTheme, normalThemeUrl, safeThemeUrl } from '../src/theme/safeTheme.js';
import { SlotBoundary } from '../src/theme/SlotBoundary.js';
import { applyTheme, THEME_CACHE_KEY, THEME_CSS_CACHE_KEY, THEME_LINK_ID } from '../src/theme/theme.js';
import { ThemeNotice } from '../src/theme/ThemeNotice.js';
import { getThemeNotice, getThemeNoticeEntry, setThemeNotice } from '../src/theme/themeNoticeStore.js';

const link = () => document.getElementById(THEME_LINK_ID);

// Setzt die Suche der aktuellen URL
function setSearch(search: string) {
  window.history.replaceState(null, '', `/board${search}#x`);
}

afterEach(() => {
  cleanup();
  setSearch('');
  localStorage.clear();
  link()?.remove();
  setThemeNotice(undefined);
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.themeMode;
  vi.restoreAllMocks();
});

describe('safe-theme in der URL', () => {
  it('erkennt den Parameter in jeder Form', () => {
    expect(isSafeTheme('?safe-theme')).toBe(true);
    expect(isSafeTheme('?safe-theme=1')).toBe(true);
    expect(isSafeTheme('?x=1&safe-theme')).toBe(true);
    expect(isSafeTheme('?x=1')).toBe(false);
    expect(isSafeTheme('')).toBe(false);
  });

  it('nimmt ohne Angabe die aktuelle URL', () => {
    setSearch('?a=2&safe-theme');
    expect(isSafeTheme()).toBe(true);
  });

  it('baut die URLs hin und zurück, Pfad, Hash und andere Parameter bleiben', () => {
    const safe = safeThemeUrl('http://localhost/board?a=2#x');
    expect(safe).toBe('http://localhost/board?a=2&safe-theme#x');
    expect(normalThemeUrl(safe)).toBe('http://localhost/board?a=2#x');
    expect(normalThemeUrl('http://localhost/board?safe-theme=1')).toBe('http://localhost/board');
  });
});

describe('applyTheme im abgesicherten Modus', () => {
  it('setzt nur hell/dunkel, keine theme.css und lässt die Einstellung stehen', () => {
    localStorage.setItem(THEME_CACHE_KEY, 'ocean');
    localStorage.setItem(THEME_CSS_CACHE_KEY, '/themes/ocean/theme.css');
    setSearch('?safe-theme');
    applyTheme('ocean', [{ id: 'ocean' }]);
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(document.head.querySelector('link[href="/themes/ocean/theme.css"]')).toBeNull();
    expect(localStorage.getItem(THEME_CACHE_KEY)).toBe('ocean');
    expect(localStorage.getItem(THEME_CSS_CACHE_KEY)).toBe('/themes/ocean/theme.css');
  });

  it('folgt dem dunklen System', () => {
    vi.stubGlobal('matchMedia', () => ({ matches: true }));
    setSearch('?safe-theme=1');
    applyTheme('ocean');
    expect(document.documentElement.dataset.theme).toBe('dark');
    vi.unstubAllGlobals();
  });

  it('entfernt einen schon gesetzten Theme-Link', () => {
    applyTheme('ocean', [{ id: 'ocean' }]);
    expect(link()).not.toBeNull();
    setSearch('?safe-theme');
    applyTheme('ocean', [{ id: 'ocean' }]);
    expect(link()).toBeNull();
  });
});

describe('main.js im abgesicherten Modus', () => {
  it('importiert nichts und setzt einen Info-Hinweis', async () => {
    localStorage.setItem(THEME_CACHE_KEY, 'ocean');
    setSearch('?safe-theme');
    const importModule = vi.fn();
    const result = await loadActiveThemeModule({ importModule, fetchManifest: vi.fn() });
    expect(result.status).toBe('skipped');
    expect(importModule).not.toHaveBeenCalled();
    expect(getThemeNoticeEntry()?.kind).toBe('info');
  });
});

describe('Hinweisbalken', () => {
  it('bietet bei einem Fehler „Abgesichert neu laden“', () => {
    const assign = vi.fn();
    render(<ThemeNotice assign={assign} />);
    act(() => setThemeNotice('Theme kaputt'));
    fireEvent.click(screen.getByRole('button', { name: 'Abgesichert neu laden' }));
    expect(assign).toHaveBeenCalledWith(`${window.location.origin}/board?safe-theme#x`);
  });

  it('bietet im abgesicherten Modus „Normal starten“', () => {
    setSearch('?a=2&safe-theme');
    const assign = vi.fn();
    render(<ThemeNotice assign={assign} />);
    act(() => setThemeNotice({ kind: 'info', message: 'Abgesicherter Modus' }));
    expect(screen.queryByRole('button', { name: 'Abgesichert neu laden' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Normal starten' }));
    expect(assign).toHaveBeenCalledWith(`${window.location.origin}/board?a=2#x`);
  });
});

describe('gescheiterter Slot', () => {
  it('setzt einen Fehlerhinweis', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    function Boom(): never {
      throw new Error('kaputt');
    }
    render(
      <SlotBoundary fallback={<span>Standard</span>} resetKey="ocean">
        <Boom />
      </SlotBoundary>,
    );
    expect(screen.getByText('Standard')).toBeTruthy();
    expect(getThemeNoticeEntry()?.kind).toBe('error');
    expect(getThemeNotice()).toContain('ocean');
  });
});

describe('index.html im abgesicherten Modus', () => {
  it('setzt keine theme.css und zeigt hell/dunkel', () => {
    const html = readFileSync(join(import.meta.dirname, '..', 'index.html'), 'utf8');
    const bootScript = /<script>([\s\S]*?)<\/script>/.exec(html)![1];
    localStorage.setItem('harness-theme-mode', 'ocean');
    localStorage.setItem('harness-theme-css', '/themes/ocean/theme.css');
    setSearch('?x=1&safe-theme');
    new Function(bootScript)();
    expect(link()).toBeNull();
    expect(document.documentElement.dataset.theme).toBe('light');
  });
});
