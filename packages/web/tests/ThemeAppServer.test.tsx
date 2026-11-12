// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import { ApiProvider } from '../src/api/ApiContext.js';
import { serverUrl } from '../src/api/nexusBridge.js';
import type { ThemeManifest } from '../src/api/generated/themes.js';
import type { StoreIndexEntryDto } from '../src/api/types.js';
import { ThemeStore } from '../src/components/settings/ThemeStore.js';
import { ThemesPanel } from '../src/components/settings/ThemesPanel.js';
import { SNIPPETS_LINK_ID, applySnippets } from '../src/theme/snippets.js';
import { THEME_CSS_CACHE_KEY, THEME_LINK_ID, applyTheme } from '../src/theme/theme.js';
import { fakeApi } from './helpers/fakeApi.js';

// In der Desktop-App kommt die Oberfläche aus den Dateien der App (z. B. http://tauri.localhost).
// Themes, Schnipsel und Vorschaubilder liegen aber beim lokalen Server; ohne dessen Adresse davor
// landen die Anfragen bei der App, die für jede unbekannte Adresse ihre index.html liefert.
const BASE = 'http://127.0.0.1:4810';

beforeEach(() => {
  window.__NEXUS__ = { base: BASE, token: 'geheim' };
});

afterEach(() => {
  cleanup();
  delete window.__NEXUS__;
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.themeMode;
  document.head.innerHTML = '';
  window.localStorage.clear();
});

const themeLinks = () => document.head.querySelectorAll<HTMLLinkElement>(`link#${THEME_LINK_ID}`);

describe('serverUrl', () => {
  it('setzt in der App die Adresse des Servers davor', () => {
    expect(serverUrl('/themes/ocean/theme.css')).toBe(`${BASE}/themes/ocean/theme.css`);
  });

  it('lässt die Adresse im Browser unverändert', () => {
    delete window.__NEXUS__;
    expect(serverUrl('/themes/ocean/theme.css')).toBe('/themes/ocean/theme.css');
  });
});

describe('Themes in der Desktop-App', () => {
  it('bindet die theme.css vom lokalen Server ein, genau einmal', () => {
    applyTheme('ocean', ['ocean']);
    applyTheme('ocean', ['ocean']);
    expect(themeLinks()).toHaveLength(1);
    expect(themeLinks()[0].getAttribute('href')).toBe(`${BASE}/themes/ocean/theme.css`);
  });

  it('merkt sich die theme.css ohne Serveradresse, weil der Port mit jedem Start wechselt', () => {
    applyTheme('ocean', ['ocean']);
    expect(window.localStorage.getItem(THEME_CSS_CACHE_KEY)).toBe('/themes/ocean/theme.css');
  });

  it('lädt die CSS-Schnipsel vom lokalen Server', () => {
    applySnippets();
    const link = document.getElementById(SNIPPETS_LINK_ID) as HTMLLinkElement;
    expect(link.getAttribute('href')).toMatch(/^http:\/\/127\.0\.0\.1:4810\/snippets\/active\.css\?v=/);
  });

  it('zeigt das Vorschaubild eines installierten Themes vom lokalen Server', async () => {
    const manifest: ThemeManifest = {
      id: 'ocean', name: 'Theme ocean', version: '1.2.0', author: 'Ada', sdkVersion: '^1.0', colorScheme: 'dark', startColor: '#123456', preview: 'assets/p.png', slots: [], permissions: { network: [] },
    };
    const settings = { notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false };
    render(<ApiProvider api={fakeApi({ getUserSettings: async () => settings, listThemes: async () => [manifest] } as never)}><ThemesPanel /></ApiProvider>);
    const list = await screen.findByRole('list', { name: 'Themes' });
    const ocean = await within(list).findByRole('listitem', { name: 'Theme ocean' });
    expect(within(ocean).getByRole('img').getAttribute('src')).toBe(`${BASE}/themes/ocean/assets/p.png`);
  });

  it('holt das Vorschaubild eines Store-Eintrags über den lokalen Server', async () => {
    const entry: StoreIndexEntryDto = {
      repoId: 'r 1', repoUrl: 'https://example.com/nexus-themes.json', id: 'forest', name: 'Forest', version: '1.0.0', sdkVersion: '^1.0',
      downloadUrl: 'https://example.com/forest.nexus', sha256: 'x', preview: 'https://example.com/f.png', compatible: true,
    };
    const api = {
      getStoreIndex: vi.fn(async () => ({ entries: [entry], repos: [{ id: 'r 1', url: entry.repoUrl, ok: true, entryCount: 1, skipped: [] }] })),
      getStoreUpdates: vi.fn(async () => ({ updates: [], errors: [] })),
    };
    render(<ApiProvider api={fakeApi(api as never)}><ThemeStore installed={[]} onInstalled={() => {}} /></ApiProvider>);
    const image = await screen.findByAltText('Vorschau: Forest');
    // Die fremde Adresse des Index steht nie im Bild: die CSP lässt Bilder nur vom lokalen Server zu
    expect(image.getAttribute('src')).toBe(`${BASE}/api/themes/store/preview?repoId=r%201&id=forest&v=1.0.0`);
  });
});
