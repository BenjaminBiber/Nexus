// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { App } from '../src/App.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import { WIN95_MAXIMIZED_KEY } from '../src/components/win95/Win95Frame.js';
import { fakeApi } from './helpers/fakeApi.js';

const SETTINGS = { notifications: 'off', heroImages: false, claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false };

beforeEach(() => {
  document.documentElement.dataset.theme = 'win95';
  window.localStorage.removeItem(WIN95_MAXIMIZED_KEY);
});

afterEach(() => {
  cleanup();
  delete document.documentElement.dataset.theme;
});

// Rendert die ganze App im Schema „Windows 95“
function openApp(path: string) {
  return render(
    <ApiProvider api={fakeApi({ getUserSettings: async () => ({ ...SETTINGS, theme: 'win95' }) })}>
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
    expect(screen.getByRole('region', { name: 'AI Harness – Sessions' })).toBeTruthy();
    expect(screen.getByRole('button', { name: /AI Harness - Sessions/ }).getAttribute('aria-pressed')).toBe('true');
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

  it('minimiert das Fenster in die Taskleiste und holt es über den Task-Knopf zurück', async () => {
    openApp('/p/p1/sessions');
    await userEvent.click(await screen.findByRole('button', { name: 'Minimieren' }));
    expect(screen.queryByRole('region', { name: 'AI Harness – Sessions' })).toBeNull();
    const task = screen.getByRole('button', { name: /AI Harness - Sessions/ });
    expect(task.getAttribute('aria-pressed')).toBe('false');
    await userEvent.click(task);
    expect(screen.getByRole('region', { name: 'AI Harness – Sessions' })).toBeTruthy();
  });

  it('wechselt zwischen maximiert und Fenster und merkt sich die Wahl', async () => {
    openApp('/p/p1/sessions');
    await userEvent.click(await screen.findByRole('button', { name: 'Wiederherstellen' }));
    expect(screen.getByRole('region', { name: 'AI Harness – Sessions' }).className).not.toContain('maximized');
    expect(window.localStorage.getItem(WIN95_MAXIMIZED_KEY)).toBe('false');
    await userEvent.click(screen.getByRole('button', { name: 'Maximieren' }));
    expect(screen.getByRole('region', { name: 'AI Harness – Sessions' }).className).toContain('maximized');
  });
});
