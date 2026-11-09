// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ApiError } from '../src/api/ApiError.js';
import type { ThemeManifest } from '../src/api/generated/themes.js';
import { ThemesPanel } from '../src/components/settings/ThemesPanel.js';
import { describeThemeError } from '../src/components/settings/themeErrors.js';
import { getThemeNoticeEntry, setThemeNotice } from '../src/theme/themeNoticeStore.js';
import { fakeApi } from './helpers/fakeApi.js';
import { stubReload } from './helpers/reload.js';
import { themeFile } from './helpers/zip.js';
afterEach(() => {
  cleanup();
  setThemeNotice(undefined);
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.themeMode;
});
const location = stubReload();

const manifest = (id: string, extra: Partial<ThemeManifest> = {}): ThemeManifest => ({
  id, name: `Theme ${id}`, version: '1.2.0', author: 'Ada', sdkVersion: '^1.0', colorScheme: 'dark', startColor: '#123456', preview: null, slots: [], permissions: { network: [] }, ...extra,
});
const settings = (theme: string) => ({ notifications: 'off', heroImages: false, theme, claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false });

function open(overrides: Record<string, unknown>, theme = 'system') {
  let current = settings(theme);
  const saveUserSettings = vi.fn(async (patch: object) => (current = { ...current, ...patch }));
  render(<ApiProvider api={fakeApi({ getUserSettings: async () => current, saveUserSettings, ...overrides } as never)}><ThemesPanel /></ApiProvider>);
  return { saveUserSettings };
}
const tiles = () => screen.findByRole('list', { name: 'Themes' });

describe('describeThemeError', () => {
  it('nennt bei Manifestfehlern die Felder', () => {
    const error = new ApiError('theme-manifest-invalid', 'x', 422, undefined, { errors: [{ field: 'id', message: 'ist ungültig' }, { field: '', message: 'unlesbar' }] });
    expect(describeThemeError(error)).toEqual({ message: 'Die manifest.json des Themes ist fehlerhaft.', fields: ['id: ist ungültig', 'unlesbar'] });
  });
  it('erklärt Paket- und Formatfehler und fällt sonst auf die Meldung zurück', () => {
    expect(describeThemeError(new ApiError('theme-package-invalid', 'Datei fehlt: theme.css', 422)).message).toBe('Die Datei ist kein gültiges Theme-Paket: Datei fehlt: theme.css');
    expect(describeThemeError(new ApiError('bad-request', 'kaputt', 400)).message).toBe('Die Datei konnte nicht übertragen werden: kaputt');
    expect(describeThemeError(new Error('Netz weg'))).toEqual({ message: 'Netz weg', fields: [] });
  });
});

describe('Darstellung › Themes', () => {
  it('zeigt Kacheln mit Name, Autor, Version und Vorschau oder Farbvorschau', async () => {
    open({ listThemes: async () => [manifest('ocean', { preview: 'assets/p.png' }), manifest('nacht', { slots: ['Shell'] })] });
    const list = await tiles();
    const ocean = within(list).getByRole('listitem', { name: 'Theme ocean' });
    expect(within(ocean).getByText('Ada · v1.2.0')).toBeTruthy();
    expect(within(ocean).getByRole('img').getAttribute('src')).toBe('/themes/ocean/assets/p.png');
    expect(within(ocean).getByText('Dunkel')).toBeTruthy();
    const nacht = within(list).getByRole('listitem', { name: 'Theme nacht' });
    expect(within(nacht).queryByRole('img')).toBeNull();
    expect(within(nacht).getByText('Mit Code')).toBeTruthy();
    expect(within(list).getByRole('listitem', { name: 'System' })).toBeTruthy();
  });

  it('markiert das aktive Theme und wendet ein anderes an', async () => {
    const { saveUserSettings } = open({ listThemes: async () => [manifest('ocean')] }, 'ocean');
    const list = await tiles();
    await waitFor(() => expect(within(list).getByRole('listitem', { name: 'Theme ocean' }).getAttribute('aria-current')).toBe('true'));
    expect(within(list).getByRole('button', { name: 'Theme ocean in Verwendung' })).toHaveProperty('disabled', true);
    fireEvent.click(within(list).getByRole('button', { name: 'Hell anwenden' }));
    await waitFor(() => expect(saveUserSettings).toHaveBeenCalledWith({ theme: 'light' }));
    await waitFor(() => expect(within(list).getByRole('listitem', { name: 'Hell' }).getAttribute('aria-current')).toBe('true'));
  });

  it('filtert nach Herkunft und sucht nach Name oder Autor', async () => {
    open({ listThemes: async () => [manifest('ocean'), manifest('wald', { author: 'Grace' })] });
    const list = await tiles();
    const filter = screen.getByRole('group', { name: 'Filter' });
    expect(within(filter).getByRole('button', { name: /Installiert/ }).textContent).toContain('2');
    fireEvent.click(within(filter).getByRole('button', { name: /Installiert/ }));
    expect(within(list).getAllByRole('listitem').map((li) => li.getAttribute('aria-label'))).toEqual(['Theme ocean', 'Theme wald']);
    fireEvent.click(within(filter).getByRole('button', { name: /Eingebaut/ }));
    expect(within(list).getAllByRole('listitem').map((li) => li.getAttribute('aria-label'))).toEqual(['System', 'Hell', 'Dunkel']);
    fireEvent.click(within(filter).getByRole('button', { name: /Alle/ }));
    fireEvent.change(screen.getByRole('searchbox', { name: 'Themes durchsuchen' }), { target: { value: 'grace' } });
    expect(within(list).getAllByRole('listitem').map((li) => li.getAttribute('aria-label'))).toEqual(['Theme wald']);
    fireEvent.change(screen.getByRole('searchbox', { name: 'Themes durchsuchen' }), { target: { value: 'gibt es nicht' } });
    expect(screen.getByText('Keine Themes für diesen Filter.')).toBeTruthy();
  });

  it('setzt auf den Standard „System“ zurück', async () => {
    const { saveUserSettings } = open({ listThemes: async () => [manifest('ocean')] }, 'ocean');
    await tiles();
    const reset = screen.getByRole('button', { name: 'Auf Standard zurücksetzen' }) as HTMLButtonElement;
    await waitFor(() => expect(reset.disabled).toBe(false));
    fireEvent.click(reset);
    await waitFor(() => expect(saveUserSettings).toHaveBeenCalledWith({ theme: 'system' }));
    await waitFor(() => expect(reset.disabled).toBe(true));
  });

  it('lädt eine .nexus-Datei hoch und zeigt das neue Theme', async () => {
    const list = [manifest('ocean')];
    const installTheme = vi.fn(async () => { list.push(manifest('neu')); return manifest('neu'); });
    open({ listThemes: async () => [...list], installTheme });
    await tiles();
    fireEvent.click(screen.getByRole('button', { name: 'Theme installieren' }));
    const dialog = await screen.findByRole('dialog', { name: 'Theme installieren' });
    const input = within(dialog).getByLabelText('Aus Datei installieren') as HTMLInputElement;
    expect(input.accept).toBe('.nexus');
    const file = await themeFile(manifest('neu'));
    fireEvent.change(input, { target: { files: [file] } });
    expect(installTheme).not.toHaveBeenCalled();
    fireEvent.click(await within(dialog).findByRole('button', { name: 'Installieren' }));
    await waitFor(() => expect(installTheme).toHaveBeenCalledWith(file));
    expect(await screen.findByRole('listitem', { name: 'Theme neu' })).toBeTruthy();
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Theme installieren' })).toBeNull());
  });

  it('zeigt Manifestfehler mit Feldern und Paketfehler verständlich an', async () => {
    const installTheme = vi.fn()
      .mockRejectedValueOnce(new ApiError('theme-manifest-invalid', 'x', 422, undefined, { errors: [{ field: 'version', message: 'ist kein semver' }] }))
      .mockRejectedValueOnce(new ApiError('theme-package-invalid', 'keine ZIP-Datei', 422));
    open({ listThemes: async () => [], installTheme });
    await tiles();
    fireEvent.click(screen.getByRole('button', { name: 'Theme installieren' }));
    const input = await screen.findByLabelText('Aus Datei installieren');
    fireEvent.change(input, { target: { files: [await themeFile(manifest('a'))] } });
    fireEvent.click(await screen.findByRole('button', { name: 'Installieren' }));
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('Die manifest.json des Themes ist fehlerhaft.');
    expect(within(alert).getByText('version: ist kein semver')).toBeTruthy();
    fireEvent.change(input, { target: { files: [await themeFile(manifest('b'))] } });
    fireEvent.click(await screen.findByRole('button', { name: 'Installieren' }));
    expect(await screen.findByText('Die Datei ist kein gültiges Theme-Paket: keine ZIP-Datei')).toBeTruthy();
  });

  it('zeigt im Installieren-Dialog die Themes der Stores und schließt mit Esc', async () => {
    open({ listThemes: async () => [], getStoreIndex: async () => ({ entries: [{ repoId: 'r1', repoUrl: 'https://x.example/i.json', id: 'ocean', name: 'Ozean', version: '1.0.0', sdkVersion: '^1.0', downloadUrl: 'x', sha256: 'y', compatible: true }], repos: [{ id: 'r1', url: 'https://x.example/i.json', ok: true, entryCount: 1, skipped: [] }] }) });
    await tiles();
    expect(screen.queryByLabelText('Store durchsuchen')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Theme installieren' }));
    const dialog = await screen.findByRole('dialog', { name: 'Theme installieren' });
    expect(await within(dialog).findByText('Ozean')).toBeTruthy();
    expect(within(dialog).getByLabelText('Store durchsuchen')).toBeTruthy();
    fireEvent.keyDown(dialog, { key: 'Escape' });
    expect(screen.queryByRole('dialog', { name: 'Theme installieren' })).toBeNull();
  });

  it('verwaltet die Theme-Stores in einem Dialog neben der Suche', async () => {
    const addStoreRepo = vi.fn(async (url: string) => ({ id: 'r1', url, addedAt: '2026-01-01' }));
    open({ listThemes: async () => [], addStoreRepo });
    await tiles();
    expect(screen.queryByLabelText('Repo-URL')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Theme-Stores verwalten' }));
    const dialog = await screen.findByRole('dialog', { name: 'Theme-Stores' });
    fireEvent.change(within(dialog).getByLabelText('Repo-URL'), { target: { value: 'https://x.example/i.json' } });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Hinzufügen' }));
    await waitFor(() => expect(addStoreRepo).toHaveBeenCalledWith('https://x.example/i.json'));
    expect(await within(dialog).findByText('https://x.example/i.json')).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Schließen' }));
    expect(screen.queryByRole('dialog', { name: 'Theme-Stores' })).toBeNull();
  });

  it('fragt vor dem Entfernen nach; Abbrechen ändert nichts, Bestätigen entfernt', async () => {
    let list = [manifest('ocean')];
    const removeTheme = vi.fn(async (id: string) => { list = list.filter((m) => m.id !== id); });
    open({ listThemes: async () => [...list], removeTheme });
    const grid = await tiles();
    expect(within(grid).queryByRole('button', { name: 'System entfernen' })).toBeNull();
    fireEvent.click(within(grid).getByRole('button', { name: 'Theme ocean entfernen' }));
    fireEvent.click(within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Abbrechen' }));
    expect(removeTheme).not.toHaveBeenCalled();
    expect(screen.queryByRole('alertdialog')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Theme ocean entfernen' }));
    fireEvent.click(within(screen.getByRole('alertdialog', { name: 'Theme „Theme ocean“ entfernen?' })).getByRole('button', { name: 'Endgültig entfernen' }));
    await waitFor(() => expect(screen.queryByRole('listitem', { name: 'Theme ocean' })).toBeNull());
    expect(removeTheme).toHaveBeenCalledWith('ocean');
  });

  it('schaltet auf System, wenn das aktive Theme entfernt wird', async () => {
    let list = [manifest('ocean')];
    const { saveUserSettings } = open({ listThemes: async () => [...list], removeTheme: async () => { list = []; } }, 'ocean');
    fireEvent.click(within(await tiles()).getByRole('button', { name: 'Theme ocean entfernen' }));
    fireEvent.click(screen.getByRole('button', { name: 'Endgültig entfernen' }));
    await waitFor(() => expect(saveUserSettings).toHaveBeenCalledWith({ theme: 'system' }));
    await waitFor(() => expect(screen.getByRole('listitem', { name: 'System' }).getAttribute('aria-current')).toBe('true'));
  });

  it('lädt beim Wechsel auf ein Code-Theme neu und zeigt den Hinweis', async () => {
    document.documentElement.dataset.theme = 'dark';
    const { saveUserSettings } = open({ listThemes: async () => [manifest('minecraft', { slots: ['Shell'] }), manifest('pastel')] }, 'dark');
    const list = await tiles();
    fireEvent.click(within(list).getByRole('button', { name: 'Theme minecraft anwenden' }));
    await waitFor(() => expect(saveUserSettings).toHaveBeenCalledWith({ theme: 'minecraft' }));
    await waitFor(() => expect(location.reload).toHaveBeenCalledTimes(1));
    expect(getThemeNoticeEntry()).toEqual({ kind: 'info', message: 'Theme wird geladen …' });

    // Zurück auf ein eingebautes Theme: ebenfalls neu laden, denn das Code-Theme geht
    location.reload.mockClear();
    fireEvent.click(within(list).getByRole('button', { name: 'Dunkel anwenden' }));
    await waitFor(() => expect(location.reload).toHaveBeenCalledTimes(1));

    // Reines CSS-Theme: kein Neuladen
    location.reload.mockClear();
    fireEvent.click(within(list).getByRole('button', { name: 'Theme pastel anwenden' }));
    await waitFor(() => expect(saveUserSettings).toHaveBeenCalledWith({ theme: 'pastel' }));
    expect(location.reload).not.toHaveBeenCalled();
  });

  it('meldet einen Ladefehler, eingebaute Modi bleiben wählbar', async () => {
    const { saveUserSettings } = open({ listThemes: async () => { throw new Error('Server weg'); } });
    expect(await screen.findByText('Installierte Themes konnten nicht geladen werden: Server weg')).toBeTruthy();
    const dark = screen.getByRole('button', { name: 'Dunkel anwenden' }) as HTMLButtonElement;
    await waitFor(() => expect(dark.disabled).toBe(false));
    fireEvent.click(dark);
    await waitFor(() => expect(saveUserSettings).toHaveBeenCalledWith({ theme: 'dark' }));
  });
});
