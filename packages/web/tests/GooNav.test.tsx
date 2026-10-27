// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render } from '@testing-library/react';
import { MemoryRouter, NavLink } from 'react-router-dom';
import { GooNav } from '../src/components/goo/GooNav.js';

function nav() {
  return render(
    <MemoryRouter initialEntries={['/docs']}>
      <GooNav className="nav" aria-label="Hauptnavigation">
        <NavLink to="/board">Board</NavLink>
        <NavLink to="/docs">Docs</NavLink>
      </GooNav>
    </MemoryRouter>,
  );
}

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe('GooNav', () => {
  it('zeichnet die Markierung ohne matchMedia nicht selbst, die Links behalten die Hervorhebung', () => {
    const { container } = nav();
    expect(container.querySelector('.goo-nav-layer')).toBeNull();
    expect(container.querySelector('a.active')?.textContent).toBe('Docs');
  });

  it('legt unter die Links eine Goo-Ebene mit Kopf, Schwanz und Steg', () => {
    vi.stubGlobal('matchMedia', () => ({ matches: false }));
    const { container, getByLabelText } = nav();
    expect(getByLabelText('Hauptnavigation').classList.contains('goo-nav')).toBe(true);
    expect(container.querySelectorAll('.goo-nav-layer .goo-nav-blob')).toHaveLength(3);
  });

  it('bleibt bei reduzierter Bewegung bei der gewöhnlichen Hervorhebung', () => {
    vi.stubGlobal('matchMedia', () => ({ matches: true }));
    const { container } = nav();
    expect(container.querySelector('.goo-nav-layer')).toBeNull();
  });
});
