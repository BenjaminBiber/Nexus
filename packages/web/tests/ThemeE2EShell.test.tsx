// @vitest-environment jsdom
// Ende-zu-Ende mit dem eingecheckten Test-Theme e2e-shell: Installieren über die Einstellungen, Auswählen,
// Neustart mit main.js, eigene Shell, Entfernen und abgesicherter Modus
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { App } from '../src/App.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { ThemeManifest } from '../src/api/generated/themes.js';
import type { UserSettingsDto } from '../src/api/types.js';
import { loadActiveThemeModule } from '../src/theme/loadThemeModule.js';
import { getTheme, unregisterTheme } from '../src/theme/registry.js';
import { THEME_CACHE_KEY, THEME_LINK_ID } from '../src/theme/theme.js';
import { setThemeNotice } from '../src/theme/themeNoticeStore.js';
import { inspectThemePackage } from '../src/themes/inspectPackage.js';
import { fakeApi } from './helpers/fakeApi.js';
import { stubReload } from './helpers/reload.js';

stubReload();

const FIXTURES = join(import.meta.dirname, 'fixtures', 'themes');
const SOURCE = join(FIXTURES, 'e2e-shell');
const manifest = JSON.parse(readFileSync(join(SOURCE, 'manifest.json'), 'utf8')) as ThemeManifest;

// Das eingecheckte Paket als Datei, wie sie der Datei-Dialog liefert
const packageFile = () => new File([readFileSync(join(FIXTURES, 'e2e-shell.nexus'))], 'e2e-shell.nexus');

// Lädt main.js der Quelle statt über den Server (Adresse im Variablen-Import, damit TypeScript keine Typen sucht)
const mainUrl = pathToFileURL(join(SOURCE, 'main.js')).href;
const importMain = vi.fn(async (_url: string) => import(/* @vite-ignore */ mainUrl));

// Server-Attrappe mit Speicher für installierte Themes und Einstellungen
function themeServer() {
  const base = fakeApi();
  let installed: ThemeManifest[] = [];
  let settings: UserSettingsDto | undefined;
  const api = {
    getSettings: async () => ({ project: { id: 'p1', name: 'Demo', path: '/demo' }, token: 't' }),
    getUserSettings: async () => (settings ??= await base.getUserSettings()),
    saveUserSettings: vi.fn(async (patch: Partial<UserSettingsDto>) => (settings = { ...(await base.getUserSettings()), ...settings, ...patch })),
    listThemes: async () => installed,
    installTheme: vi.fn(async (file: Blob) => {
      const { manifest: m } = await inspectThemePackage(file);
      installed = [...installed.filter((t) => t.id !== m.id), m];
      return m;
    }),
    removeTheme: vi.fn(async (id: string) => {
      installed = installed.filter((t) => t.id !== id);
    }),
  };
  return { api: fakeApi(api as never), calls: api, installed: () => installed };
}

function openApp(api: ReturnType<typeof fakeApi>, path: string) {
  return render(
    <ApiProvider api={api}>
      <MemoryRouter initialEntries={[path]}>
        <App />
      </MemoryRouter>
    </ApiProvider>,
  );
}

// Neustart der App: main.js des gemerkten Themes laden wie in main.tsx
function restart(search = '') {
  cleanup();
  return loadActiveThemeModule({ importModule: importMain, fetchManifest: async () => manifest, search });
}

beforeEach(() => {
  vi.spyOn(console, 'warn').mockImplementation(() => {});
});

afterEach(() => {
  cleanup();
  unregisterTheme('e2e-shell');
  localStorage.clear();
  setThemeNotice(undefined);
  document.getElementById(THEME_LINK_ID)?.remove();
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.themeMode;
  importMain.mockClear();
  vi.restoreAllMocks();
});

describe('Test-Theme e2e-shell', () => {
  it('Paket lesen: Manifest wie die Quelle, mit Code', async () => {
    const read = await inspectThemePackage(packageFile());
    expect(read.manifest).toEqual(expect.objectContaining({ id: 'e2e-shell', slots: ['Shell'], sdkVersion: manifest.sdkVersion }));
    expect(read.hasCode).toBe(true);
  });

  it('installieren, auswählen, eigene Shell nach Neustart, entfernen → Standard', async () => {
    const server = themeServer();

    // Installieren über die Einstellungen: Dialog mit Code-Hinweis, danach als Kachel anwendbar
    openApp(server.api, '/p/p1/settings#appearance');
    fireEvent.click(await screen.findByRole('button', { name: 'Theme installieren' }));
    fireEvent.change(await screen.findByLabelText('Aus Datei installieren'), { target: { files: [packageFile()] } });
    const dialog = await screen.findByRole('alertdialog', { name: 'Theme installieren?' });
    expect(within(dialog).getByText('E2E Shell')).toBeTruthy();
    expect(within(dialog).getByText('Dieses Theme führt Code aus')).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Installieren' }));
    fireEvent.click(await screen.findByRole('button', { name: 'E2E Shell anwenden' }));
    expect(server.calls.installTheme).toHaveBeenCalledTimes(1);
    expect(server.installed().map((t) => t.id)).toEqual(['e2e-shell']);
    await waitFor(() => expect(server.calls.saveUserSettings).toHaveBeenCalledWith({ theme: 'e2e-shell' }));
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe('e2e-shell'));
    expect(localStorage.getItem(THEME_CACHE_KEY)).toBe('e2e-shell');
    expect(document.getElementById(THEME_LINK_ID)?.getAttribute('href')).toBe('/themes/e2e-shell/theme.css');

    // Neustart: main.js wird geladen und die eigene Shell ersetzt die Seitenleiste
    expect(await restart()).toEqual({ status: 'loaded', themeId: 'e2e-shell' });
    expect(importMain).toHaveBeenCalledWith('/themes/e2e-shell/main.js');
    openApp(server.api, '/p/p1/sessions');
    expect(await screen.findByRole('heading', { name: 'Sessions', level: 1 })).toBeTruthy();
    const shell = screen.getByTestId('e2e-shell');
    expect(within(shell).getByText('E2E-Shell')).toBeTruthy();
    expect(within(shell).getByRole('heading', { name: 'Sessions', level: 1 })).toBeTruthy();
    expect(within(shell).getByRole('button', { name: /Suchen/ })).toBeTruthy();
    expect(document.querySelector('.shell-sidebar')).toBeNull();

    // Entfernen über die Navigation der eigenen Shell: Theme abgemeldet, Standard-Shell mit hell/dunkel
    fireEvent.click(within(screen.getByRole('navigation', { name: 'E2E-Navigation' })).getByRole('link', { name: 'Einstellungen' }));
    fireEvent.click(await screen.findByRole('link', { name: /Darstellung/ }));
    fireEvent.click(await screen.findByRole('button', { name: 'E2E Shell entfernen' }));
    fireEvent.click(within(screen.getByRole('alertdialog', { name: 'Theme „E2E Shell“ entfernen?' })).getByRole('button', { name: 'Endgültig entfernen' }));
    await waitFor(() => expect(server.calls.removeTheme).toHaveBeenCalledWith('e2e-shell'));
    await waitFor(() => expect(screen.queryByTestId('e2e-shell')).toBeNull());
    expect(document.querySelector('.shell-sidebar')).toBeTruthy();
    expect(getTheme('e2e-shell')).toBeUndefined();
    expect(server.calls.saveUserSettings).toHaveBeenLastCalledWith({ theme: 'system' });
    expect(['light', 'dark']).toContain(document.documentElement.dataset.theme);
    expect(document.getElementById(THEME_LINK_ID)).toBeNull();
  });

  it('?safe-theme lädt keinen Code und zeigt die Standard-Shell', async () => {
    const server = themeServer();
    await server.api.installTheme(packageFile());
    await server.api.saveUserSettings({ theme: 'e2e-shell' });
    localStorage.setItem(THEME_CACHE_KEY, 'e2e-shell');
    window.history.replaceState(null, '', '/?safe-theme');
    try {
      expect(await restart('?safe-theme')).toEqual({ status: 'skipped', themeId: 'e2e-shell' });
      expect(importMain).not.toHaveBeenCalled();
      openApp(server.api, '/p/p1/sessions');
      expect(await screen.findByRole('heading', { name: 'Sessions', level: 1 })).toBeTruthy();
      expect(screen.queryByTestId('e2e-shell')).toBeNull();
      expect(document.querySelector('.shell-sidebar')).toBeTruthy();
    } finally {
      window.history.replaceState(null, '', '/');
    }
  });
});
