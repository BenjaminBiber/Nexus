// @vitest-environment jsdom
// Rahmen des Pakets in der ganzen App: installiert (listThemes) und angemeldet wie nach dem Laden der main.js
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { App } from '../../../packages/web/src/App.js';
import { ApiProvider } from '../../../packages/web/src/api/ApiContext.js';
import { registerTheme, unregisterTheme } from '../../../packages/web/src/theme/registry.js';
import { THEME_LINK_ID } from '../../../packages/web/src/theme/theme.js';
import { fakeApi } from '../../../packages/web/tests/helpers/fakeApi.js';
import theme from '../src/main.js';
import { WIN95_MAXIMIZED_KEY } from '../src/Win95Frame.js';

const SETTINGS = { notifications: 'off', heroImages: false, claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false };
const manifest = JSON.parse(readFileSync(join(import.meta.dirname, '..', 'manifest.json'), 'utf8'));

beforeEach(() => {
  registerTheme(theme);
  document.documentElement.dataset.theme = 'win95';
  window.localStorage.removeItem(WIN95_MAXIMIZED_KEY);
});

afterEach(() => {
  cleanup();
  unregisterTheme('win95');
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.themeMode;
  document.getElementById(THEME_LINK_ID)?.remove();
});

// Rendert die ganze App im Schema „Windows 95“
function openApp(path: string) {
  return render(
    <ApiProvider api={fakeApi({ getUserSettings: async () => ({ ...SETTINGS, theme: 'win95' }), listThemes: async () => [manifest] } as never)}>
      <MemoryRouter initialEntries={[path]}>
        <App />
      </MemoryRouter>
    </ApiProvider>,
  );
}

describe('Rahmen „Windows 95“', () => {
  it('zeigt die Navigation als Desktop-Symbole und die Seite als Fenster mit Titel', async () => {
    openApp('/p/p1/sessions');
    const nav = await screen.findByRole('navigation', { name: 'Hauptnavigation' });
    expect(within(nav).getByRole('link', { name: 'Board' }).getAttribute('href')).toBe('/p/p1/board');
    expect(screen.getByRole('region', { name: 'Nexus – Sessions' })).toBeTruthy();
    expect(screen.getByRole('button', { name: /Nexus - Sessions/ }).getAttribute('aria-pressed')).toBe('true');
  });

  it('öffnet das Startmenü, wandert mit den Pfeiltasten und schließt mit Esc', async () => {
    openApp('/p/p1/sessions');
    const start = await screen.findByRole('button', { name: 'Start' });
    await userEvent.click(start);
    const menu = screen.getByRole('menu', { name: 'Start' });
    const items = within(menu).getAllByRole('menuitem');
    expect(items.map((item) => item.textContent)).toEqual(['Board', 'Dashboard', 'Sessions', 'Anleitung', 'Einstellungen', 'Suchen …']);
    expect(document.activeElement).toBe(items[0]);
    await userEvent.keyboard('{ArrowUp}');
    expect(document.activeElement).toBe(items[5]);
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('menu')).toBeNull();
    expect(document.activeElement).toBe(start);
  });

  it('hebt den aktiven Eintrag hervor, wechselt über das Startmenü den Bereich und passt den Titel an', async () => {
    openApp('/p/p1/sessions');
    await userEvent.click(await screen.findByRole('button', { name: 'Start' }));
    const menu = screen.getByRole('menu', { name: 'Start' });
    expect(within(menu).getByRole('menuitem', { name: 'Sessions' }).className).toContain('active');
    await userEvent.click(within(menu).getByRole('menuitem', { name: 'Anleitung' }));
    expect(screen.queryByRole('menu')).toBeNull();
    expect(await screen.findByRole('region', { name: 'Nexus – Anleitung' })).toBeTruthy();
    expect(screen.getByRole('button', { name: /Nexus - Anleitung/ })).toBeTruthy();
  });

  it('öffnet über „Suchen …“ im Startmenü die Suche', async () => {
    openApp('/p/p1/sessions');
    await userEvent.click(await screen.findByRole('button', { name: 'Start' }));
    await userEvent.click(screen.getByRole('menuitem', { name: 'Suchen …' }));
    expect(screen.queryByRole('menu')).toBeNull();
    expect(await screen.findByRole('dialog')).toBeTruthy();
  });

  it('minimiert das Fenster in die Taskleiste und holt es über den Task-Knopf zurück', async () => {
    openApp('/p/p1/sessions');
    await userEvent.click(await screen.findByRole('button', { name: 'Minimieren' }));
    expect(screen.queryByRole('region', { name: 'Nexus – Sessions' })).toBeNull();
    const task = screen.getByRole('button', { name: /Nexus - Sessions/ });
    expect(task.getAttribute('aria-pressed')).toBe('false');
    await userEvent.click(task);
    expect(screen.getByRole('region', { name: 'Nexus – Sessions' })).toBeTruthy();
  });

  it('wechselt zwischen maximiert und Fenster und merkt sich die Wahl', async () => {
    openApp('/p/p1/sessions');
    await userEvent.click(await screen.findByRole('button', { name: 'Wiederherstellen' }));
    expect(screen.getByRole('region', { name: 'Nexus – Sessions' }).className).not.toContain('maximized');
    expect(window.localStorage.getItem(WIN95_MAXIMIZED_KEY)).toBe('false');
    await userEvent.click(screen.getByRole('button', { name: 'Maximieren' }));
    expect(screen.getByRole('region', { name: 'Nexus – Sessions' }).className).toContain('maximized');
  });
});
