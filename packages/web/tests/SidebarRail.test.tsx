// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import type { ShellParts } from '@nexus/theme-sdk';
import { DefaultShell } from '../src/components/DefaultShell.js';
import { LAPTOP_QUERY, SIDEBAR_KEY } from '../src/components/sidebarRail.js';

const parts: ShellParts = {
  search: <button type="button">Suchen</button>,
  boardSwitcher: null,
  usage: <section aria-label="Claude-Nutzungslimits" />,
  navItems: {},
  outlet: null,
  connection: null,
  status: null,
};

function renderShell() {
  render(
    <MemoryRouter>
      <DefaultShell pid="p1" nav={<a href="#b">Board</a>} content={<div className="shell-content" />} parts={parts} />
    </MemoryRouter>,
  );
  return {
    aside: () => document.querySelector('aside.shell-sidebar')!,
    toggle: () => screen.getByRole('button', { name: /klappen/ }),
  };
}

// Fensterbreite vortäuschen: nur die Laptop-Abfrage trifft zu, wenn `laptop` gesetzt ist
function viewport(laptop: boolean) {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches: laptop && query === LAPTOP_QUERY,
    addEventListener: () => {},
    removeEventListener: () => {},
  }));
}

beforeEach(() => localStorage.clear());
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe('Seitenleiste als Symbolleiste', () => {
  it('ist auf großen Bildschirmen offen und lässt sich von Hand einklappen', () => {
    viewport(false);
    const { aside, toggle } = renderShell();
    expect(aside().classList.contains('is-rail')).toBe(false);
    expect(toggle().getAttribute('aria-expanded')).toBe('true');
    expect(toggle().textContent).toBe('Einklappen');
    fireEvent.click(toggle());
    expect(aside().classList.contains('is-rail')).toBe(true);
    expect(toggle().textContent).toBe('Ausklappen');
    expect(localStorage.getItem(SIDEBAR_KEY)).toBe('collapsed');
  });

  it('ist auf Laptop-Breite von sich aus eingeklappt und lässt sich dauerhaft ausklappen', () => {
    viewport(true);
    const { aside, toggle } = renderShell();
    expect(aside().classList.contains('is-rail')).toBe(true);
    expect(toggle().getAttribute('aria-expanded')).toBe('false');
    fireEvent.click(toggle());
    expect(aside().classList.contains('is-rail')).toBe(false);
    expect(localStorage.getItem(SIDEBAR_KEY)).toBe('expanded');
  });

  it('folgt wieder der Fensterbreite, wenn man auf deren Vorgabe zurückstellt', () => {
    viewport(true);
    const { aside, toggle } = renderShell();
    fireEvent.click(toggle());
    fireEvent.click(toggle());
    expect(aside().classList.contains('is-rail')).toBe(true);
    expect(localStorage.getItem(SIDEBAR_KEY)).toBeNull();
  });

  it('merkt sich die Wahl über das Neuladen hinweg', () => {
    viewport(true);
    localStorage.setItem(SIDEBAR_KEY, 'expanded');
    const { aside } = renderShell();
    expect(aside().classList.contains('is-rail')).toBe(false);
  });

  it('kommt ohne matchMedia aus (offen)', () => {
    const { aside } = renderShell();
    expect(aside().classList.contains('is-rail')).toBe(false);
  });
});
