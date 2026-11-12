// @vitest-environment jsdom
// Rahmen des Pakets in der ganzen App: installiert (listThemes) und angemeldet wie nach dem Laden der main.js
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { App } from '../../../packages/web/src/App.js';
import { ApiProvider } from '../../../packages/web/src/api/ApiContext.js';
import { registerTheme, unregisterTheme } from '../../../packages/web/src/theme/registry.js';
import { THEME_LINK_ID } from '../../../packages/web/src/theme/theme.js';
import { fakeApi } from '../../../packages/web/tests/helpers/fakeApi.js';
import theme from '../src/main.js';

const SETTINGS = { notifications: 'off', heroImages: false, claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false };
const manifest = JSON.parse(readFileSync(join(import.meta.dirname, '..', 'manifest.json'), 'utf8'));

beforeEach(() => {
  registerTheme(theme);
  document.documentElement.dataset.theme = 'terminal';
  document.documentElement.dataset.themeMode = 'terminal';
});

afterEach(() => {
  cleanup();
  unregisterTheme('terminal');
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.themeMode;
  document.getElementById(THEME_LINK_ID)?.remove();
});

// Rendert die ganze App im Schema „Terminal“; gibt die Aufzeichnung der gespeicherten Einstellungen zurück
function openApp(path: string, themeSettings: Record<string, boolean | string> = {}) {
  const saveUserSettings = vi.fn(async (patch: Record<string, unknown>) => ({ ...SETTINGS, theme: 'terminal', ...patch }));
  render(
    <ApiProvider api={fakeApi({ getUserSettings: async () => ({ ...SETTINGS, theme: 'terminal' }), listThemes: async () => [manifest], saveUserSettings, getThemeSettings: async () => themeSettings } as never)}>
      <MemoryRouter initialEntries={[path]}>
        <App />
      </MemoryRouter>
    </ApiProvider>,
  );
  return { saveUserSettings };
}

const prompt = () => screen.findByRole('textbox', { name: 'Befehl (help zeigt alle Befehle)' });

// Tippt einen Befehl in die Prompt-Zeile und schickt ihn ab
async function type(command: string) {
  await userEvent.click(await prompt());
  await userEvent.keyboard(`${command}{Enter}`);
}

describe('Rahmen „Terminal“', () => {
  it('zeigt Prompt mit Board und Pfad, die Navigation als Liste und die Statuszeile', async () => {
    openApp('/p/p1/sessions');
    await prompt();
    await waitFor(() => expect(screen.getByText('nexus@sprint')).toBeTruthy());
    expect(screen.getByText('~/sessions')).toBeTruthy();
    const nav = screen.getByRole('navigation', { name: 'Hauptnavigation' });
    expect(within(nav).getByRole('link', { name: 'Board' }).getAttribute('href')).toBe('/p/p1/board');
    expect(nav.textContent).toContain('$ ls --icons ~/');
    expect(screen.getByText('SESSIONS')).toBeTruthy();
  });

  it('wechselt mit cd den Bereich', async () => {
    openApp('/p/p1/sessions');
    await type('cd anleitung');
    await waitFor(() => expect(screen.getByText('~/guide')).toBeTruthy());
    expect(screen.getByText('GUIDE')).toBeTruthy();
  });

  it('meldet unbekannte Befehle in der Ausgabe; Esc schließt sie', async () => {
    openApp('/p/p1/sessions');
    await type('make me a sandwich');
    const log = await screen.findByRole('log', { name: 'Ausgabe' });
    expect(log.textContent).toContain('command not found: make');
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('log')).toBeNull();
  });

  it('ergänzt mit Tab und holt mit ↑ den letzten Befehl zurück', async () => {
    openApp('/p/p1/sessions');
    const input = (await prompt()) as HTMLInputElement;
    await userEvent.click(input);
    await userEvent.keyboard('he{Tab}');
    expect(input.value).toBe('help ');
    await userEvent.keyboard('{Enter}');
    expect((await screen.findByRole('log')).textContent).toContain('Bereich wechseln');
    await userEvent.keyboard('{ArrowUp}');
    expect(input.value).toBe('help');
  });

  it('springt mit „/“ in die Prompt-Zeile', async () => {
    openApp('/p/p1/sessions');
    const input = await prompt();
    (document.activeElement as HTMLElement | null)?.blur();
    await userEvent.keyboard('/');
    expect(document.activeElement).toBe(input);
  });

  it('öffnet mit find die Suche des Hosts', async () => {
    openApp('/p/p1/sessions');
    await type('find');
    expect(await screen.findByRole('dialog')).toBeTruthy();
  });

  it('listet mit board die Boards des Hosts, das gewählte mit Stern', async () => {
    openApp('/p/p1/sessions');
    await waitFor(async () => expect(screen.getByText('nexus@sprint')).toBeTruthy());
    await type('board');
    const log = await screen.findByRole('log', { name: 'Ausgabe' });
    expect(log.textContent).toContain('* sprint');
    expect(log.textContent).toContain('nächster');
  });

  it('speichert mit theme das Farbschema über die Einstellungen; danach gilt der Rahmen des neuen Schemas', async () => {
    const { saveUserSettings } = openApp('/p/p1/sessions');
    await type('theme dunkel');
    await waitFor(() => expect(saveUserSettings).toHaveBeenCalledWith({ theme: 'dark' }));
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe('dark'));
    expect(screen.queryByRole('textbox', { name: 'Befehl (help zeigt alle Befehle)' })).toBeNull();
  });

  it('zeigt im Status-Segment, ob der letzte Befehl geklappt hat', async () => {
    openApp('/p/p1/sessions');
    await prompt();
    expect(document.querySelector('.term-seg-status')).toBeNull();
    await type('gibtsnicht');
    await waitFor(() => expect(screen.getByTitle('Letzter Befehl fehlgeschlagen').textContent).toBe('✗'));
    await type('ls');
    await waitFor(() => expect(screen.getByTitle('Letzter Befehl erfolgreich').textContent).toBe('✓'));
  });

  it('legt die Einstellungen des Themes als data-term-* auf <html> und räumt sie beim Wechsel weg', async () => {
    openApp('/p/p1/sessions', { palette: 'dracula', crt: true, icons: 'unicode' });
    await prompt();
    const root = document.documentElement;
    await waitFor(() => expect(root.getAttribute('data-term-palette')).toBe('dracula'));
    expect(root.getAttribute('data-term-crt')).toBe('on');
    expect(root.getAttribute('data-term-font')).toBe('jetbrains');
    expect(root.getAttribute('data-term-icons')).toBe('unicode');
    await type('theme dunkel');
    await waitFor(() => expect(root.getAttribute('data-term-palette')).toBeNull());
  });

  it('zeigt Symbole vor den Einträgen der Navigation, mit „keine“ gar keine', async () => {
    openApp('/p/p1/sessions');
    const nav = await screen.findByRole('navigation', { name: 'Hauptnavigation' });
    await waitFor(() => expect(nav.querySelector('[data-glyph="sessions"]')?.textContent).toBe(''));
    expect(nav.textContent).toContain('$ ls --icons ~/');
    cleanup();
    openApp('/p/p1/sessions', { icons: 'off' });
    const bare = await screen.findByRole('navigation', { name: 'Hauptnavigation' });
    await waitFor(() => expect(bare.textContent).toContain('$ ls ~/'));
    expect(bare.querySelector('.term-glyph')).toBeNull();
  });

  it('färbt die Statuszeile nach dem Bereich und schaltet getippte Ausgabe per Einstellung', async () => {
    openApp('/p/p1/sessions', { typeOutput: true });
    await prompt();
    expect(document.querySelector('.term-statusline')?.getAttribute('data-section')).toBe('sessions');
    await type('help');
    await waitFor(() => expect(screen.getByRole('log').className).toContain('term-typing'));
  });
});
