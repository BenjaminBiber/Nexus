// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { ApiProvider } from '../src/api/ApiContext.js';
import { ApiError } from '../src/api/ApiError.js';
import type { ThemeManifest } from '../src/api/generated/themes.js';
import { AppearancePanel } from '../src/components/AppearancePanel.js';
import { describeThemeError } from '../src/components/settings/themeErrors.js';
import { fakeApi } from './helpers/fakeApi.js';
afterEach(cleanup);

const manifest = (id: string, extra: Partial<ThemeManifest> = {}): ThemeManifest => ({
  id, name: `Theme ${id}`, version: '1.2.0', author: 'Ada', sdkVersion: '^1.0', colorScheme: 'dark', startColor: '#123456', preview: null, slots: [], permissions: { network: [] }, ...extra,
});
const settings = (theme: string) => ({ notifications: 'off', heroImages: false, theme, claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false });

function open(overrides: Record<string, unknown>, theme = 'system') {
  let current = settings(theme);
  const saveUserSettings = vi.fn(async (patch: object) => (current = { ...current, ...patch }));
  render(<ApiProvider api={fakeApi({ getUserSettings: async () => current, saveUserSettings, ...overrides } as never)}><AppearancePanel /></ApiProvider>);
  return { saveUserSettings };
}
const tiles = () => screen.findByRole('list', { name: 'Installierte Themes' });

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

describe('Installierte Themes', () => {
  it('zeigt Kacheln mit Name, Autor, Version und Vorschau oder Platzhalter', async () => {
    open({ listThemes: async () => [manifest('ocean', { preview: 'assets/p.png' }), manifest('nacht')] });
    const list = await tiles();
    const ocean = within(list).getByRole('listitem', { name: 'Theme ocean' });
    expect(within(ocean).getByText('Ada · Version 1.2.0')).toBeTruthy();
    expect(within(ocean).getByRole('img').getAttribute('src')).toBe('/themes/ocean/assets/p.png');
    const nacht = within(list).getByRole('listitem', { name: 'Theme nacht' });
    expect(within(nacht).queryByRole('img')).toBeNull();
  });

  it('lädt eine .nexus-Datei hoch und zeigt das neue Theme', async () => {
    const list = [manifest('ocean')];
    const installTheme = vi.fn(async () => { list.push(manifest('neu')); return manifest('neu'); });
    open({ listThemes: async () => [...list], installTheme });
    await tiles();
    const input = screen.getByLabelText('Theme hochladen') as HTMLInputElement;
    expect(input.accept).toBe('.nexus');
    const file = new File(['zip'], 'neu.nexus');
    fireEvent.change(input, { target: { files: [file] } });
    await waitFor(() => expect(installTheme).toHaveBeenCalledWith(file));
    expect(await screen.findByRole('listitem', { name: 'Theme neu' })).toBeTruthy();
  });

  it('zeigt Manifestfehler mit Feldern und Paketfehler verständlich an', async () => {
    const installTheme = vi.fn()
      .mockRejectedValueOnce(new ApiError('theme-manifest-invalid', 'x', 422, undefined, { errors: [{ field: 'version', message: 'ist kein semver' }] }))
      .mockRejectedValueOnce(new ApiError('theme-package-invalid', 'keine ZIP-Datei', 422));
    open({ listThemes: async () => [], installTheme });
    await tiles();
    const input = screen.getByLabelText('Theme hochladen');
    fireEvent.change(input, { target: { files: [new File(['a'], 'a.nexus')] } });
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('Die manifest.json des Themes ist fehlerhaft.');
    expect(within(alert).getByText('version: ist kein semver')).toBeTruthy();
    fireEvent.change(input, { target: { files: [new File(['b'], 'b.nexus')] } });
    expect(await screen.findByText('Die Datei ist kein gültiges Theme-Paket: keine ZIP-Datei')).toBeTruthy();
  });

  it('fragt vor dem Entfernen nach; Abbrechen ändert nichts, Bestätigen entfernt', async () => {
    let list = [manifest('ocean')];
    const removeTheme = vi.fn(async (id: string) => { list = list.filter((m) => m.id !== id); });
    open({ listThemes: async () => [...list], removeTheme });
    fireEvent.click(within(await tiles()).getByRole('button', { name: 'Theme ocean entfernen' }));
    fireEvent.click(within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Abbrechen' }));
    expect(removeTheme).not.toHaveBeenCalled();
    expect(screen.queryByRole('alertdialog')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Theme ocean entfernen' }));
    fireEvent.click(within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Endgültig entfernen' }));
    await waitFor(() => expect(screen.queryByRole('listitem', { name: 'Theme ocean' })).toBeNull());
    expect(removeTheme).toHaveBeenCalledWith('ocean');
  });

  it('schaltet auf System, wenn das aktive Theme entfernt wird', async () => {
    let list = [manifest('ocean')];
    const { saveUserSettings } = open({ listThemes: async () => [...list], removeTheme: async () => { list = []; } }, 'ocean');
    fireEvent.click(within(await tiles()).getByRole('button', { name: 'Theme ocean entfernen' }));
    fireEvent.click(screen.getByRole('button', { name: 'Endgültig entfernen' }));
    await waitFor(() => expect(saveUserSettings).toHaveBeenCalledWith({ theme: 'system' }));
    await waitFor(() => expect((screen.getByRole('radio', { name: 'System' }) as HTMLInputElement).checked).toBe(true));
  });

  it('meldet einen Ladefehler, eingebaute Modi bleiben wählbar', async () => {
    const { saveUserSettings } = open({ listThemes: async () => { throw new Error('Server weg'); } });
    expect(await screen.findByText('Installierte Themes konnten nicht geladen werden: Server weg')).toBeTruthy();
    const dark = screen.getByRole('radio', { name: 'Dunkel' }) as HTMLInputElement;
    await waitFor(() => expect(dark.disabled).toBe(false));
    fireEvent.click(dark);
    await waitFor(() => expect(saveUserSettings).toHaveBeenCalledWith({ theme: 'dark' }));
  });
});
