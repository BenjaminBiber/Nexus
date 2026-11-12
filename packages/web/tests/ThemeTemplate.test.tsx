// @vitest-environment jsdom
// Vorlage themes/_template: aktiviert ersetzt ihre Beispiel-Shell die Standard-Shell
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { cleanup, render, screen, within } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { App } from '../src/App.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { ThemeManifest } from '../src/api/generated/themes.js';
import { loadActiveThemeModule } from '../src/theme/loadThemeModule.js';
import { unregisterTheme } from '../src/theme/registry.js';
import { THEME_CACHE_KEY, THEME_LINK_ID } from '../src/theme/theme.js';
import { fakeApi } from './helpers/fakeApi.js';

const TEMPLATE = join(import.meta.dirname, '..', '..', '..', 'themes', '_template');
const manifest = JSON.parse(readFileSync(join(TEMPLATE, 'manifest.json'), 'utf8')) as ThemeManifest;

// Statt dist/main.js vom Server die Quelle der Vorlage (Vitest übersetzt TSX und löst SDK/React auf)
const mainUrl = pathToFileURL(join(TEMPLATE, 'src', 'main.tsx')).href;
const importMain = vi.fn(async (_url: string) => import(/* @vite-ignore */ mainUrl));

afterEach(() => {
  cleanup();
  unregisterTheme('my-theme');
  localStorage.clear();
  document.getElementById(THEME_LINK_ID)?.remove();
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.themeMode;
});

describe('Theme-Vorlage', () => {
  it('aktiviert: Beispiel-Shell mit Seite und Suchknopf statt Standard-Shell', async () => {
    localStorage.setItem(THEME_CACHE_KEY, 'my-theme');
    expect(await loadActiveThemeModule({ importModule: importMain, fetchManifest: async () => manifest })).toEqual({
      status: 'loaded',
      themeId: 'my-theme',
    });
    expect(importMain).toHaveBeenCalledWith('/themes/my-theme/main.js');

    const base = fakeApi();
    const api = fakeApi({
      getSettings: async () => ({ project: { id: 'p1', name: 'Demo', path: '/demo' }, token: 't' }),
      getUserSettings: async () => ({ ...(await base.getUserSettings()), theme: 'my-theme' }),
      listThemes: async () => [manifest],
    } as never);
    render(
      <ApiProvider api={api}>
        <MemoryRouter initialEntries={['/p/p1/sessions']}>
          <App />
        </MemoryRouter>
      </ApiProvider>,
    );

    expect(await screen.findByRole('heading', { name: 'Sessions', level: 1 })).toBeTruthy();
    const shell = screen.getByTestId('template-shell');
    expect(within(shell).getByText('Mein Theme')).toBeTruthy();
    expect(within(shell).getByRole('heading', { name: 'Sessions', level: 1 })).toBeTruthy();
    expect(within(shell).getByRole('button', { name: /Suchen/ })).toBeTruthy();
    expect(document.querySelector('.shell-sidebar')).toBeNull();
  });
});
