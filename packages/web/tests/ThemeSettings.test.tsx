// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { useThemeSettings } from '@nexus/theme-sdk';
import type { ThemeSettingsSchema } from '@nexus/theme-sdk';
import { App } from '../src/App.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import { AppearancePanel } from '../src/components/AppearancePanel.js';
import { registerTheme, unregisterTheme } from '../src/theme/registry.js';
import { THEME_SETTINGS_EVENT, resolveThemeSettings } from '../src/theme/themeSettings.js';
import { fakeApi } from './helpers/fakeApi.js';

const schema = {
  sounds: { type: 'boolean', label: 'Sounds', default: true },
  style: { type: 'select', label: 'Stil', options: [{ value: 'pixel', label: 'Pixel' }, { value: 'smooth', label: 'Weich' }], default: 'pixel' },
} satisfies ThemeSettingsSchema;

afterEach(() => {
  cleanup();
  unregisterTheme('test-sounds');
  delete document.documentElement.dataset.theme;
});

// Fake-Server, der die gespeicherten Werte je Theme hält
function serverApi(initial: Record<string, boolean | string> = {}) {
  let stored = initial;
  const getThemeSettings = vi.fn(async () => stored);
  const saveThemeSettings = vi.fn(async (_id: string, values: Record<string, boolean | string>) => (stored = values));
  return { api: fakeApi({ getThemeSettings, saveThemeSettings }), saveThemeSettings };
}

function showTheme() {
  registerTheme({ id: 'test-sounds', slots: {}, settingsSchema: schema });
  document.documentElement.dataset.theme = 'test-sounds';
}

describe('resolveThemeSettings', () => {
  it('nimmt passende gespeicherte Werte, sonst den Default, und lässt unbekannte Schlüssel weg', () => {
    expect(resolveThemeSettings(schema, {})).toEqual({ sounds: true, style: 'pixel' });
    expect(resolveThemeSettings(schema, { sounds: false, style: 'smooth' })).toEqual({ sounds: false, style: 'smooth' });
    expect(resolveThemeSettings(schema, { sounds: 'ja', style: 'kariert', fremd: true })).toEqual({ sounds: true, style: 'pixel' });
    expect(resolveThemeSettings(undefined, { sounds: false })).toEqual({});
  });
});

describe('Einstellungen des Themes unter Darstellung', () => {
  it('zeigt Schalter und Auswahl mit Defaults und speichert Änderungen', async () => {
    showTheme();
    const { api, saveThemeSettings } = serverApi();
    render(<ApiProvider api={api}><AppearancePanel /></ApiProvider>);
    expect(await screen.findByText('Einstellungen des Themes')).toBeTruthy();
    const sounds = screen.getByRole('checkbox', { name: 'Sounds' }) as HTMLInputElement;
    await waitFor(() => expect(sounds.disabled).toBe(false));
    expect(sounds.checked).toBe(true);
    expect((screen.getByRole('combobox', { name: 'Stil' }) as HTMLSelectElement).value).toBe('pixel');

    await userEvent.click(sounds);
    expect(saveThemeSettings).toHaveBeenCalledWith('test-sounds', { sounds: false, style: 'pixel' });
    await waitFor(() => expect((screen.getByRole('checkbox', { name: 'Sounds' }) as HTMLInputElement).checked).toBe(false));

    await userEvent.selectOptions(screen.getByRole('combobox', { name: 'Stil' }), 'smooth');
    expect(saveThemeSettings).toHaveBeenLastCalledWith('test-sounds', { sounds: false, style: 'smooth' });
  });

  it('zeigt gespeicherte Werte vom Server nach dem Laden', async () => {
    showTheme();
    const { api } = serverApi({ sounds: false });
    render(<ApiProvider api={api}><AppearancePanel /></ApiProvider>);
    await waitFor(() => expect((screen.getByRole('checkbox', { name: 'Sounds' }) as HTMLInputElement).checked).toBe(false));
  });

  it('zeigt die Sektion für ein Theme ohne Schema nicht', async () => {
    document.documentElement.dataset.theme = 'light';
    render(<ApiProvider api={fakeApi()}><AppearancePanel /></ApiProvider>);
    await screen.findByRole('radio', { name: 'Dunkel' });
    expect(screen.queryByText('Einstellungen des Themes')).toBeNull();
  });
});

describe('useThemeSettings im Nexus-Host', () => {
  it('liefert die gespeicherten Werte und nach einer Änderung den neuen Wert', async () => {
    function SoundShell() {
      const { values } = useThemeSettings<typeof schema>();
      return <p>{`sounds:${values.sounds}`}</p>;
    }
    registerTheme({ id: 'test-sounds', slots: { Shell: SoundShell }, settingsSchema: schema });
    document.documentElement.dataset.theme = 'test-sounds';
    const { api } = serverApi({ sounds: false });
    const base = fakeApi();
    const userSettings = { ...(await base.getUserSettings()), theme: 'test-sounds' as never };
    const full = fakeApi({ ...api, getUserSettings: async () => userSettings } as never);
    render(
      <ApiProvider api={full}>
        <MemoryRouter initialEntries={['/p/p1/sessions']}>
          <App />
        </MemoryRouter>
      </ApiProvider>,
    );
    expect(await screen.findByText('sounds:false')).toBeTruthy();
    await act(async () => {
      await api.saveThemeSettings('test-sounds', { sounds: true, style: 'pixel' });
      window.dispatchEvent(new CustomEvent(THEME_SETTINGS_EVENT, { detail: { themeId: 'test-sounds' } }));
    });
    expect(await screen.findByText('sounds:true')).toBeTruthy();
  });
});
