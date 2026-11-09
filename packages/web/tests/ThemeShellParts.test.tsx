// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { BoardSwitcher, NavLink, Outlet, SearchButton, UsageMeter, useNav } from '@nexus/theme-sdk';
import type { ShellProps } from '@nexus/theme-sdk';
import { App } from '../src/App.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import { registerTheme, unregisterTheme } from '../src/theme/registry.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(() => {
  cleanup();
  unregisterTheme('test-parts');
  delete document.documentElement.dataset.theme;
});

// Shell eines Themes nur aus ShellProps und Bausteinen des SDK
function PartsShell({ parts }: ShellProps) {
  const nav = useNav();
  return (
    <div data-testid="parts-shell">
      <header>
        <BoardSwitcher />
        <SearchButton />
        <UsageMeter />
        {parts.status}
      </header>
      <nav aria-label="Theme-Navigation">{nav.map((item) => <NavLink key={item.id} id={item.id} />)}</nav>
      <aside aria-label="Standard-Links">{parts.navItems.docs}</aside>
      <main>
        {parts.connection}
        <Outlet />
      </main>
    </div>
  );
}

function openApp(path: string) {
  registerTheme({ id: 'test-parts', slots: { Shell: PartsShell } });
  document.documentElement.dataset.theme = 'test-parts';
  const base = fakeApi();
  const api = fakeApi({ getUserSettings: async () => ({ ...(await base.getUserSettings()), theme: 'test-parts' as never }) });
  return render(
    <ApiProvider api={api}>
      <MemoryRouter initialEntries={[path]}>
        <App />
      </MemoryRouter>
    </ApiProvider>,
  );
}

describe('Vollständige Shell aus Bausteinen des SDK', () => {
  it('zeigt Navigation, Boardwechsel, Suche, Verbrauch und die Seite der Route', async () => {
    openApp('/p/p1/sessions');
    expect(await screen.findByRole('heading', { name: 'Sessions', level: 1 })).toBeTruthy();
    const shell = screen.getByTestId('parts-shell');
    expect(shell.querySelector('.shell-sidebar')).toBeNull();
    const nav = screen.getByRole('navigation', { name: 'Theme-Navigation' });
    const hrefs = [...nav.querySelectorAll('a')].map((a) => a.getAttribute('href'));
    expect(hrefs).toEqual(['board', 'inbox', 'dashboard', 'sessions', 'questions', 'docs', 'guide', 'settings'].map((id) => `/p/p1/${id}`));
    expect(screen.getByRole('link', { name: 'Sessions' }).getAttribute('aria-current')).toBe('page');
    expect(await screen.findByRole('button', { name: /^Board wählen/ })).toBeTruthy();
    expect(screen.getByRole('complementary', { name: 'Standard-Links' }).querySelector('a')?.getAttribute('href')).toBe('/p/p1/docs');
  });

  it('Klick auf einen Nav-Eintrag wechselt die Route, die Suche öffnet sich', async () => {
    openApp('/p/p1/sessions');
    await screen.findByRole('heading', { name: 'Sessions', level: 1 });
    const nav = screen.getByRole('navigation', { name: 'Theme-Navigation' });
    await userEvent.click(nav.querySelector('a[href="/p/p1/dashboard"]')!);
    expect(await screen.findByRole('heading', { name: 'Dashboard', level: 1 })).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: /Suchen/ }));
    expect(await screen.findByRole('heading', { name: 'Suche über alle Boards' })).toBeTruthy();
  });
});
