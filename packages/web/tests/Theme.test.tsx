// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { UserSettingsDto } from '../src/api/types.js';
import { InterfacePanel } from '../src/components/settings/InterfacePanel.js';
import { ThemesPanel } from '../src/components/settings/ThemesPanel.js';
import { MermaidBlock } from '../src/markdown/MermaidBlock.js';
import { registerTheme, unregisterTheme } from '../src/theme/registry.js';
import { ThemeSync } from '../src/theme/ThemeSync.js';
import { THEME_CACHE_KEY, THEME_CSS_CACHE_KEY, THEME_LINK_ID, THEME_START_COLOR_KEY, applyTheme, currentTheme, displayedTheme, needsReload, normalizeThemeId, resolveTheme } from '../src/theme/theme.js';
import { fakeApi } from './helpers/fakeApi.js';
import { stubReload } from './helpers/reload.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg viewBox="0 0 10 10"></svg>' })) } }));

afterEach(() => {
  cleanup();
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.themeMode;
  document.getElementById(THEME_LINK_ID)?.remove();
  window.localStorage.clear();
});

stubReload();

const ocean = { id: 'ocean', startColor: '#123456' };
const themeLinks = () => document.head.querySelectorAll(`link#${THEME_LINK_ID}`);

describe('theme.css installierter Themes', () => {
  it('bindet die theme.css genau einmal ein und merkt sich URL und Startfarbe', () => {
    applyTheme('ocean', [ocean]);
    applyTheme('ocean', [ocean]);
    expect(themeLinks()).toHaveLength(1);
    expect(themeLinks()[0].getAttribute('href')).toBe('/themes/ocean/theme.css');
    expect((themeLinks()[0] as HTMLLinkElement).rel).toBe('stylesheet');
    expect(document.documentElement.dataset.theme).toBe('ocean');
    expect(document.documentElement.dataset.themeMode).toBe('ocean');
    expect(window.localStorage.getItem(THEME_CSS_CACHE_KEY)).toBe('/themes/ocean/theme.css');
    expect(window.localStorage.getItem(THEME_START_COLOR_KEY)).toBe('#123456');
  });

  it('ersetzt den Link beim Wechsel zwischen installierten Themes', () => {
    applyTheme('ocean', [ocean, 'wald']);
    applyTheme('wald', [ocean, 'wald']);
    expect(themeLinks()).toHaveLength(1);
    expect(themeLinks()[0].getAttribute('href')).toBe('/themes/wald/theme.css');
    expect(window.localStorage.getItem(THEME_START_COLOR_KEY)).toBeNull();
  });

  it('entfernt Link und Cache beim Wechsel auf ein eingebautes Theme', () => {
    for (const mode of ['dark', 'system']) {
      applyTheme('ocean', [ocean]);
      applyTheme(mode);
      expect(themeLinks()).toHaveLength(0);
      expect(window.localStorage.getItem(THEME_CSS_CACHE_KEY)).toBeNull();
      expect(window.localStorage.getItem(THEME_START_COLOR_KEY)).toBeNull();
    }
  });

  it('legt den Link vor die CSS-Schnipsel des Nutzers', () => {
    const snippets = document.createElement('link');
    snippets.id = 'nexus-snippets-css';
    document.head.appendChild(snippets);
    applyTheme('ocean', [ocean]);
    expect(themeLinks()[0].nextElementSibling).toBe(snippets);
    snippets.remove();
  });

  it('fällt beim Start auf „system“ zurück, wenn das gewählte Theme nicht mehr installiert ist', async () => {
    applyTheme('ocean', [ocean]);
    render(<ApiProvider api={fakeApi({ getUserSettings: async () => ({ notifications: 'off', heroImages: false, theme: 'ocean', claudeUsage: false, compactCards: false, hideNotNeeded: false }), listThemes: async () => [] } as never)}><ThemeSync /></ApiProvider>);
    await waitFor(() => expect(document.documentElement.dataset.themeMode).toBe('system'));
    expect(themeLinks()).toHaveLength(0);
    expect(window.localStorage.getItem(THEME_CSS_CACHE_KEY)).toBeNull();
  });

  it('übernimmt beim Start ein installiertes Theme mit Link', async () => {
    render(<ApiProvider api={fakeApi({ getUserSettings: async () => ({ notifications: 'off', heroImages: false, theme: 'ocean', claudeUsage: false, compactCards: false, hideNotNeeded: false }), listThemes: async () => [ocean] } as never)}><ThemeSync /></ApiProvider>);
    await waitFor(() => expect(themeLinks()).toHaveLength(1));
    expect(document.documentElement.dataset.theme).toBe('ocean');
  });

  it('behält den Link aus dem Cache, wenn die Liste der Themes nicht lädt', async () => {
    applyTheme('ocean', [ocean]);
    const listThemes = async () => {
      throw new Error('kaputt');
    };
    render(<ApiProvider api={fakeApi({ getUserSettings: async () => ({ notifications: 'off', heroImages: false, theme: 'ocean', claudeUsage: false, compactCards: false, hideNotNeeded: false }), listThemes } as never)}><ThemeSync /></ApiProvider>);
    await waitFor(() => expect(document.documentElement.dataset.themeMode).toBe('ocean'));
    expect(themeLinks()).toHaveLength(1);
    expect(window.localStorage.getItem(THEME_START_COLOR_KEY)).toBe('#123456');
  });
});

describe('Theme', () => {
  it('löst „system“ über das Betriebssystem auf', () => {
    expect(resolveTheme('system', true)).toBe('dark');
    expect(resolveTheme('system', false)).toBe('light');
    expect(resolveTheme('light', true)).toBe('light');
    expect(resolveTheme('dark', false)).toBe('dark');
    expect(resolveTheme('retro', true)).toBe('retro');
  });

  it('zeigt ein installiertes Code-Theme als eigenes Schema, für Mermaid aber hell', () => {
    applyTheme('retro', [{ id: 'retro', startColor: '#c0c0c0' }]);
    expect(document.documentElement.dataset.theme).toBe('retro');
    expect(displayedTheme()).toBe('retro');
    expect(currentTheme()).toBe('light');
  });

  it('behält die theme.css eines angemeldeten Code-Themes, wenn die Liste der installierten fehlt', () => {
    registerTheme({ id: 'retro', slots: {} });
    try {
      applyTheme('retro', [{ id: 'retro', startColor: '#c0c0c0' }]);
      applyTheme('retro');
      expect(themeLinks()[0]?.getAttribute('href')).toBe('/themes/retro/theme.css');
      expect(window.localStorage.getItem(THEME_CSS_CACHE_KEY)).toBe('/themes/retro/theme.css');
      expect(window.localStorage.getItem(THEME_START_COLOR_KEY)).toBe('#c0c0c0');
    } finally {
      unregisterTheme('retro');
    }
  });

  it('zeigt ein installiertes helles Theme als eigenes Schema', () => {
    expect(resolveTheme('pastel', true)).toBe('pastel');
    applyTheme('pastel', [{ id: 'pastel', name: 'Pastell', version: '1.0.0', author: 'Nexus', colorScheme: 'light' }] as never);
    expect(document.documentElement.dataset.theme).toBe('pastel');
    expect(displayedTheme()).toBe('pastel');
    expect(currentTheme()).toBe('light');
  });

  it('ersetzt eine unbekannte Theme-ID durch „system“', () => {
    for (const mode of ['system', 'light', 'dark']) expect(normalizeThemeId(mode, [])).toBe(mode);
    for (const mode of ['signal', 'pastel', 'win95']) {
      expect(normalizeThemeId(mode, [])).toBe('system');
      expect(normalizeThemeId(mode, [mode])).toBe(mode);
    }
    expect(normalizeThemeId('lila', [])).toBe('system');
    expect(normalizeThemeId('ocean', ['ocean'])).toBe('ocean');
    applyTheme('lila', []);
    expect(document.documentElement.dataset.themeMode).toBe('system');
    applyTheme('../x');
    expect(document.documentElement.dataset.themeMode).toBe('system');
    expect(['light', 'dark']).toContain(document.documentElement.dataset.theme);
  });

  it('zeigt installierte Themes im Panel „Themes“ und speichert ihre ID', async () => {
    const saveUserSettings = vi.fn(async (patch: Partial<UserSettingsDto>) => ({ notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, ...patch }));
    const listThemes = async () => [{ id: 'ocean', name: 'Ozean', version: '1.0.0', author: 'Ada' }, { id: 'dark', name: 'Doppelt', version: '1.0.0', author: 'Ada' }];
    render(<ApiProvider api={fakeApi({ saveUserSettings, listThemes } as never)}><ThemesPanel /></ApiProvider>);
    const ocean = await screen.findByRole('button', { name: 'Ozean anwenden' });
    expect(screen.queryByRole('button', { name: 'Doppelt anwenden' })).toBeNull();
    await userEvent.click(ocean);
    expect(saveUserSettings).toHaveBeenCalledWith({ theme: 'ocean' });
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe('ocean'));
  });

  it('zeigt die eingebauten Themes, wenn die Liste der installierten nicht lädt', async () => {
    const listThemes = async () => {
      throw new Error('kaputt');
    };
    render(<ApiProvider api={fakeApi({ listThemes })}><ThemesPanel /></ApiProvider>);
    await screen.findByRole('button', { name: 'Dunkel anwenden' });
    expect(within(screen.getByRole('list', { name: 'Themes' })).getAllByRole('listitem')).toHaveLength(3);
    expect(screen.queryByRole('button', { name: 'Signal anwenden' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Pastell anwenden' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Windows 95 anwenden' })).toBeNull();
  });

  it('bietet „Windows 95“ als installiertes Paket an und lädt seine theme.css', async () => {
    const saveUserSettings = vi.fn(async (patch: Partial<UserSettingsDto>) => ({ notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, ...patch }));
    const listThemes = async () => [{ id: 'win95', name: 'Windows 95', version: '1.0.0', author: 'Nexus', colorScheme: 'light', startColor: '#c0c0c0', slots: ['Shell', 'NavIcon', 'Progress'] }];
    render(<ApiProvider api={fakeApi({ saveUserSettings, listThemes } as never)}><ThemesPanel /></ApiProvider>);
    await userEvent.click(await screen.findByRole('button', { name: 'Windows 95 anwenden' }));
    expect(saveUserSettings).toHaveBeenCalledWith({ theme: 'win95' });
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe('win95'));
    expect(document.getElementById('nexus-theme-css')?.getAttribute('href')).toBe('/themes/win95/theme.css');
    expect(window.localStorage.getItem(THEME_START_COLOR_KEY)).toBe('#c0c0c0');
    expect(currentTheme()).toBe('light');
  });

  it('bietet „Pastell“ als installiertes Paket an und lädt seine theme.css', async () => {
    const saveUserSettings = vi.fn(async (patch: Partial<UserSettingsDto>) => ({ notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, ...patch }));
    const listThemes = async () => [{ id: 'pastel', name: 'Pastell', version: '1.0.0', author: 'Nexus', colorScheme: 'light', startColor: '#f3eadf' }];
    render(<ApiProvider api={fakeApi({ saveUserSettings, listThemes } as never)}><ThemesPanel /></ApiProvider>);
    await userEvent.click(await screen.findByRole('button', { name: 'Pastell anwenden' }));
    expect(saveUserSettings).toHaveBeenCalledWith({ theme: 'pastel' });
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe('pastel'));
    expect(document.getElementById('nexus-theme-css')?.getAttribute('href')).toBe('/themes/pastel/theme.css');
    expect(currentTheme()).toBe('light');
  });

  it('bietet „Signal“ als installiertes Paket an und lädt seine theme.css', async () => {
    const saveUserSettings = vi.fn(async (patch: Partial<UserSettingsDto>) => ({ notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, ...patch }));
    const listThemes = async () => [{ id: 'signal', name: 'Signal', version: '1.0.0', author: 'Nexus', colorScheme: 'light', startColor: '#f4f6f8' }];
    render(<ApiProvider api={fakeApi({ saveUserSettings, listThemes } as never)}><ThemesPanel /></ApiProvider>);
    await userEvent.click(await screen.findByRole('button', { name: 'Signal anwenden' }));
    expect(saveUserSettings).toHaveBeenCalledWith({ theme: 'signal' });
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe('signal'));
    expect(document.getElementById('nexus-theme-css')?.getAttribute('href')).toBe('/themes/signal/theme.css');
    expect(currentTheme()).toBe('light');
  });

  it('speichert das Farbschema im Panel „Themes“ und setzt es sofort', async () => {
    const saveUserSettings = vi.fn(async (patch: Partial<UserSettingsDto>) => ({ notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, ...patch }));
    render(<ApiProvider api={fakeApi({ saveUserSettings })}><ThemesPanel /></ApiProvider>);
    const dark = await screen.findByRole('button', { name: 'Dunkel anwenden' });
    expect(screen.getByRole('listitem', { name: 'System' }).getAttribute('aria-current')).toBe('true');
    await userEvent.click(dark);
    expect(saveUserSettings).toHaveBeenCalledWith({ theme: 'dark' });
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe('dark'));
    await waitFor(() => expect(screen.getByRole('listitem', { name: 'Dunkel' }).getAttribute('aria-current')).toBe('true'));
    expect(screen.getByRole('button', { name: 'Dunkel in Verwendung' })).toBeTruthy();
  });

  it('schaltet die kompakten Ticket-Kacheln im Panel „Oberfläche“ ein', async () => {
    const saveUserSettings = vi.fn(async (patch: Partial<UserSettingsDto>) => ({ notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, ...patch }) as UserSettingsDto);
    render(<ApiProvider api={fakeApi({ saveUserSettings })}><InterfacePanel /></ApiProvider>);
    const box = (await screen.findByRole('checkbox', { name: 'Kompakte Ticket-Kacheln' })) as HTMLInputElement;
    expect(box.checked).toBe(false);
    await userEvent.click(box);
    expect(saveUserSettings).toHaveBeenCalledWith({ compactCards: true });
    await waitFor(() => expect((screen.getByRole('checkbox', { name: 'Kompakte Ticket-Kacheln' }) as HTMLInputElement).checked).toBe(true));
  });

  it('speichert im Panel „Oberfläche“, ob die Spalte „Nicht mehr nötig“ ausgeblendet wird', async () => {
    const saveUserSettings = vi.fn(async (patch: Partial<UserSettingsDto>) => ({ notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, ...patch }));
    render(<ApiProvider api={fakeApi({ saveUserSettings })}><InterfacePanel /></ApiProvider>);
    const box = await screen.findByRole('checkbox', { name: /Nicht mehr nötig/ });
    await waitFor(() => expect((box as HTMLInputElement).disabled).toBe(false));
    await userEvent.click(box);
    expect(saveUserSettings).toHaveBeenCalledWith({ hideNotNeeded: true });
    await waitFor(() => expect((box as HTMLInputElement).checked).toBe(true));
  });

  it('speichert im Panel „Oberfläche“, ob die Spalte „In Planung“ ausgeblendet wird', async () => {
    const saveUserSettings = vi.fn(async (patch: Partial<UserSettingsDto>) => ({ notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false, ...patch }));
    render(<ApiProvider api={fakeApi({ saveUserSettings })}><InterfacePanel /></ApiProvider>);
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

describe('needsReload', () => {
  const manifests = [{ id: 'minecraft', slots: ['Shell'] }, { id: 'pastel', slots: [] }, { id: 'win95', slots: ['Shell', 'NavIcon'] }];

  it('lädt neu, wenn ein Code-Theme kommt oder geht', () => {
    expect(needsReload('dark', 'minecraft', manifests)).toBe(true);
    expect(needsReload('minecraft', 'dark', manifests)).toBe(true);
    expect(needsReload('win95', 'minecraft', manifests)).toBe(true);
  });

  it('lädt nicht neu zwischen reinen CSS- und eingebauten Themes', () => {
    expect(needsReload('dark', 'pastel', manifests)).toBe(false);
    expect(needsReload('pastel', 'light', manifests)).toBe(false);
    expect(needsReload('minecraft', 'minecraft', manifests)).toBe(false);
  });

  it('lädt ohne Manifeste nie neu', () => {
    expect(needsReload('dark', 'minecraft', [])).toBe(false);
  });
});
