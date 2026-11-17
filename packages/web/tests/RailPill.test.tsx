// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import type { ShellParts } from '@nexus/theme-sdk';
import { DefaultShell } from '../src/components/DefaultShell.js';
import { RAIL_QUERY } from '../src/components/goo/RailPill.js';
import { LAPTOP_QUERY, SIDEBAR_KEY } from '../src/components/sidebarRail.js';

const openSearch = vi.fn();

const parts: ShellParts = {
  search: (
    <button type="button" className="global-search" onClick={openSearch}>
      <span className="global-search-label">Suchen</span>
    </button>
  ),
  boardSwitcher: null,
  usage: <section aria-label="Claude-Nutzungslimits" />,
  navItems: {},
  outlet: null,
  connection: null,
  status: null,
};

const nav = (
  <>
    <a href="#board" className="active">
      <span className="nav-label">Board</span>
    </a>
    <a href="#tasks">
      <span className="nav-label">Meine Aufgaben</span>
      <span className="nav-badge">3</span>
    </a>
  </>
);

function renderShell() {
  render(
    <MemoryRouter>
      <DefaultShell pid="p1" nav={nav} content={<div className="shell-content" />} parts={parts} />
    </MemoryRouter>,
  );
  const aside = document.querySelector<HTMLElement>('aside.shell-sidebar')!;
  return {
    aside,
    pill: () => document.querySelector<HTMLElement>('.rail-pill'),
    label: () => document.querySelector<HTMLElement>('.rail-pill-content:not(.is-ghost)')!,
    link: (name: string) => [...aside.querySelectorAll<HTMLElement>('.nav a')].find((a) => a.textContent?.startsWith(name))!,
  };
}

// Fensterbreite und Bewegungs-Vorliebe vortäuschen; ohne Bewegung springt die Pille sofort in ihre Endlage
function viewport({ rail = true, wide = true } = {}) {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches:
      (rail && query === LAPTOP_QUERY) || (wide && query === RAIL_QUERY) || query === '(prefers-reduced-motion: reduce)',
    addEventListener: () => {},
    removeEventListener: () => {},
  }));
}

beforeEach(() => {
  localStorage.clear();
  openSearch.mockReset();
  vi.useFakeTimers();
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('Pille der eingeklappten Leiste', () => {
  it('zeigt den überfahrenen Eintrag ganz, statt die Leiste aufzuklappen', () => {
    viewport();
    const { aside, pill, label, link } = renderShell();
    expect(aside.classList.contains('is-rail')).toBe(true);
    expect(pill()!.classList.contains('is-open')).toBe(false);

    fireEvent.pointerOver(link('Meine Aufgaben'));
    expect(pill()!.classList.contains('is-open')).toBe(true);
    expect(label().textContent).toBe('Meine Aufgaben3');
    expect(label().getAttribute('href')).toBe('#tasks');
    expect(label().dataset.kind).toBe('nav');
    expect(label().style.opacity).toBe('1');
    expect(link('Meine Aufgaben').hasAttribute('data-rail-pill')).toBe(true);
    // Die Leiste selbst bleibt schmal
    expect(aside.classList.contains('is-rail')).toBe(true);
  });

  it('wechselt beim Weiterfahren zum nächsten Eintrag und übernimmt dessen Zustand', () => {
    viewport();
    const { pill, label, link } = renderShell();
    fireEvent.pointerOver(link('Meine Aufgaben'));
    fireEvent.pointerOver(link('Board'));
    expect(pill()!.classList.contains('is-open')).toBe(true);
    expect(label().textContent).toBe('Board');
    expect(label().classList.contains('is-active')).toBe(true);
    expect(link('Meine Aufgaben').hasAttribute('data-rail-pill')).toBe(false);
  });

  it('zieht sich zurück, sobald der Zeiger die Leiste verlässt', () => {
    viewport();
    const { aside, pill, link } = renderShell();
    fireEvent.pointerOver(link('Board'));
    fireEvent.pointerLeave(aside);
    expect(pill()!.classList.contains('is-open')).toBe(true);
    act(() => void vi.advanceTimersByTime(200));
    expect(pill()!.classList.contains('is-open')).toBe(false);
    expect(link('Board').hasAttribute('data-rail-pill')).toBe(false);
  });

  it('reicht einen Klick auf die Kopie an den echten Knopf weiter', () => {
    viewport();
    const { aside, pill, label } = renderShell();
    fireEvent.pointerOver(aside.querySelector('.global-search')!);
    expect(label().textContent).toBe('Suchen');
    expect(label().dataset.kind).toBe('search');
    fireEvent.click(label().querySelector('.global-search-label')!);
    expect(openSearch).toHaveBeenCalledTimes(1);
    // Die Suche ist offen, die Pille steht ihr nicht im Weg
    expect(pill()!.classList.contains('is-open')).toBe(false);
  });

  it('gibt es nicht, solange die Leiste ausgeklappt ist', () => {
    viewport({ rail: false });
    const { aside, pill } = renderShell();
    expect(aside.classList.contains('is-rail')).toBe(false);
    expect(pill()).toBeNull();
  });

  it('bleibt auf schmalen Bildschirmen zu, wo die Leiste keine Symbolleiste ist', () => {
    viewport({ rail: false, wide: false });
    localStorage.setItem(SIDEBAR_KEY, 'collapsed');
    const { pill, link } = renderShell();
    fireEvent.pointerOver(link('Board'));
    expect(pill()!.classList.contains('is-open')).toBe(false);
  });
});
