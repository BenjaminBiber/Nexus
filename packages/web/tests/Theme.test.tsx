// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { UserSettingsDto } from '../src/api/types.js';
import { AppearancePanel } from '../src/components/AppearancePanel.js';
import { MermaidBlock } from '../src/markdown/MermaidBlock.js';
import { ThemeSync } from '../src/theme/ThemeSync.js';
import { THEME_CACHE_KEY, applyTheme, currentTheme, displayedTheme, normalizeThemeId, resolveTheme } from '../src/theme/theme.js';
import { fakeApi } from './helpers/fakeApi.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg viewBox="0 0 10 10"></svg>' })) } }));

afterEach(() => {
  cleanup();
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.themeMode;
});

describe('Theme', () => {
  it('löst „system“ über das Betriebssystem auf', () => {
    expect(resolveTheme('system', true)).toBe('dark');
    expect(resolveTheme('system', false)).toBe('light');
    expect(resolveTheme('light', true)).toBe('light');
    expect(resolveTheme('dark', false)).toBe('dark');
    expect(resolveTheme('win95', true)).toBe('win95');
  });

  it('zeigt „win95“ als eigenes Schema, für Mermaid aber hell', () => {
    applyTheme('win95');
    expect(document.documentElement.dataset.theme).toBe('win95');
    expect(displayedTheme()).toBe('win95');
    expect(currentTheme()).toBe('light');
  });

  it('zeigt „signal“ und „pastel“ als eigene helle Schemata', () => {
    for (const mode of ['signal', 'pastel'] as const) {
      expect(resolveTheme(mode, true)).toBe(mode);
      applyTheme(mode);
      expect(document.documentElement.dataset.theme).toBe(mode);
      expect(displayedTheme()).toBe(mode);
      expect(currentTheme()).toBe('light');
    }
  });

  it('ersetzt eine unbekannte Theme-ID durch „system“', () => {
    for (const mode of ['system', 'light', 'dark', 'signal', 'pastel', 'win95']) expect(normalizeThemeId(mode, [])).toBe(mode);
    expect(normalizeThemeId('lila', [])).toBe('system');
    expect(normalizeThemeId('ocean', ['ocean'])).toBe('ocean');
    applyTheme('lila', []);
    expect(document.documentElement.dataset.themeMode).toBe('system');
    applyTheme('../x');
    expect(document.documentElement.dataset.themeMode).toBe('system');
    expect(['light', 'dark']).toContain(document.documentElement.dataset.theme);
  });

  it('zeigt installierte Themes im Panel „Darstellung“ und speichert ihre ID', async () => {
    const saveUserSettings = vi.fn(async (patch: Partial<UserSettingsDto>) => ({ notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, ...patch }));
    const listThemes = async () => [{ id: 'ocean', name: 'Ozean', version: '1.0.0', author: 'Ada' }, { id: 'dark', name: 'Doppelt', version: '1.0.0', author: 'Ada' }];
    render(<ApiProvider api={fakeApi({ saveUserSettings, listThemes } as never)}><AppearancePanel /></ApiProvider>);
    const ocean = await screen.findByRole('radio', { name: 'Ozean' });
    expect(screen.queryByRole('radio', { name: 'Doppelt' })).toBeNull();
    await userEvent.click(ocean);
    expect(saveUserSettings).toHaveBeenCalledWith({ theme: 'ocean' });
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe('ocean'));
  });

  it('zeigt die eingebauten Themes, wenn die Liste der installierten nicht lädt', async () => {
    const listThemes = async () => {
      throw new Error('kaputt');
    };
    render(<ApiProvider api={fakeApi({ listThemes })}><AppearancePanel /></ApiProvider>);
    await screen.findByRole('radio', { name: 'Windows 95' });
    expect(screen.getAllByRole('radio')).toHaveLength(6);
  });

  it('speichert das Farbschema im Panel „Darstellung“ und setzt es sofort', async () => {
    const saveUserSettings = vi.fn(async (patch: Partial<UserSettingsDto>) => ({ notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, ...patch }));
    render(<ApiProvider api={fakeApi({ saveUserSettings })}><AppearancePanel /></ApiProvider>);
    const dark = await screen.findByRole('radio', { name: 'Dunkel' });
    expect((screen.getByRole('radio', { name: 'System' }) as HTMLInputElement).checked).toBe(true);
    await userEvent.click(dark);
    expect(saveUserSettings).toHaveBeenCalledWith({ theme: 'dark' });
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe('dark'));
    expect((screen.getByRole('radio', { name: 'Dunkel' }) as HTMLInputElement).checked).toBe(true);
  });

  it('schaltet die kompakten Ticket-Kacheln im Panel „Darstellung“ ein', async () => {
    const saveUserSettings = vi.fn(async (patch: Partial<UserSettingsDto>) => ({ notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, ...patch }) as UserSettingsDto);
    render(<ApiProvider api={fakeApi({ saveUserSettings })}><AppearancePanel /></ApiProvider>);
    const box = (await screen.findByRole('checkbox', { name: 'Kompakte Ticket-Kacheln' })) as HTMLInputElement;
    expect(box.checked).toBe(false);
    await userEvent.click(box);
    expect(saveUserSettings).toHaveBeenCalledWith({ compactCards: true });
    await waitFor(() => expect((screen.getByRole('checkbox', { name: 'Kompakte Ticket-Kacheln' }) as HTMLInputElement).checked).toBe(true));
  });

  it('speichert im Panel „Darstellung“, ob die Spalte „Nicht mehr nötig“ ausgeblendet wird', async () => {
    const saveUserSettings = vi.fn(async (patch: Partial<UserSettingsDto>) => ({ notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, ...patch }));
    render(<ApiProvider api={fakeApi({ saveUserSettings })}><AppearancePanel /></ApiProvider>);
    const box = await screen.findByRole('checkbox', { name: /Nicht mehr nötig/ });
    await waitFor(() => expect((box as HTMLInputElement).disabled).toBe(false));
    await userEvent.click(box);
    expect(saveUserSettings).toHaveBeenCalledWith({ hideNotNeeded: true });
    await waitFor(() => expect((box as HTMLInputElement).checked).toBe(true));
  });

  it('speichert im Panel „Darstellung“, ob die Spalte „In Planung“ ausgeblendet wird', async () => {
    const saveUserSettings = vi.fn(async (patch: Partial<UserSettingsDto>) => ({ notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false, ...patch }));
    render(<ApiProvider api={fakeApi({ saveUserSettings })}><AppearancePanel /></ApiProvider>);
    const box = await screen.findByRole('checkbox', { name: /In Planung/ });
    await waitFor(() => expect((box as HTMLInputElement).disabled).toBe(false));
    await userEvent.click(box);
    expect(saveUserSettings).toHaveBeenCalledWith({ hideInPlanning: true });
    await waitFor(() => expect((box as HTMLInputElement).checked).toBe(true));
  });

  it('übernimmt beim Start die gespeicherte Einstellung', async () => {
    render(<ApiProvider api={fakeApi({ getUserSettings: async () => ({ notifications: 'off', heroImages: false, theme: 'dark', claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false }) })}><ThemeSync /></ApiProvider>);
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe('dark'));
    expect(document.documentElement.dataset.themeMode).toBe('dark');
  });

  it('folgt bei „system“ live dem Wechsel im Betriebssystem', async () => {
    let dark = false;
    const listeners = new Set<() => void>();
    const original = window.matchMedia;
    window.matchMedia = ((query: string) => ({
      get matches() {
        return dark;
      },
      media: query,
      addEventListener: (_: string, fn: () => void) => listeners.add(fn),
      removeEventListener: (_: string, fn: () => void) => listeners.delete(fn),
    })) as unknown as typeof window.matchMedia;
    try {
      render(<ApiProvider api={fakeApi()}><ThemeSync /></ApiProvider>);
      await waitFor(() => expect(document.documentElement.dataset.theme).toBe('light'));
      dark = true;
      listeners.forEach((fn) => fn());
      expect(document.documentElement.dataset.theme).toBe('dark');
      expect(document.documentElement.dataset.themeMode).toBe('system');
    } finally {
      cleanup();
      window.matchMedia = original;
    }
    expect(listeners.size).toBe(0);
  });

  it('merkt sich die Einstellung für den nächsten Start', () => {
    applyTheme('dark');
    expect(window.localStorage.getItem(THEME_CACHE_KEY)).toBe('dark');
  });

  it('zeichnet Mermaid im Dunkelmodus mit dunklem Theme', async () => {
    document.documentElement.dataset.theme = 'dark';
    render(<MermaidBlock source="graph TD; A-->B" />);
    const mermaid = (await import('mermaid')).default;
    await waitFor(() => expect(mermaid.initialize).toHaveBeenCalledWith(expect.objectContaining({ theme: 'dark' })));
  });
});
