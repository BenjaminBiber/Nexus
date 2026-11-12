// @vitest-environment jsdom
// Symbole im Stil GTA V für die Slots NavIcon und Icon: gefüllte Zeichen in currentColor, für Screenreader ausgeblendet
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';
import type { IconName, NavIconName } from '@nexus/theme-sdk';
import { VIcon } from '../src/v/VIcon.js';
import { VNavIcon } from '../src/v/VNavIcon.js';

afterEach(cleanup);

const NAV: NavIconName[] = ['board', 'dashboard', 'inbox', 'sessions', 'questions', 'docs', 'search', 'guide', 'settings'];
const ICONS: IconName[] = ['doc', 'plan', 'glossary', 'adr', 'file', 'folder', 'folderOpen', 'empty', 'edit', 'comment'];

const svgOf = (container: HTMLElement) => container.querySelector('svg') as SVGSVGElement;
const markup = (container: HTMLElement) => svgOf(container).innerHTML;

function expectGlyph(container: HTMLElement) {
  const svg = svgOf(container);
  expect(svg).not.toBeNull();
  expect(svg.getAttribute('aria-hidden')).toBe('true');
  expect(svg.getAttribute('focusable')).toBe('false');
  expect(svg.getAttribute('fill')).toBe('currentColor');
  expect(svg.classList.contains('gta-v-icon')).toBe(true);
  expect(svg.querySelectorAll('path, rect, circle').length).toBeGreaterThan(0);
}

describe('VNavIcon', () => {
  it.each(NAV)('liefert für „%s“ ein eigenes Zeichen', (name) => {
    const { container } = render(<VNavIcon name={name} />);
    expectGlyph(container);
  });

  it('jedes Navigationssymbol sieht anders aus', () => {
    const shapes = NAV.map((name) => {
      const { container } = render(<VNavIcon name={name} />);
      const shape = markup(container);
      cleanup();
      return shape;
    });
    expect(new Set(shapes).size).toBe(NAV.length);
  });

  it('unbekannte Namen ergeben ein neutrales Zeichen statt eines Fehlers', () => {
    const { container } = render(<VNavIcon name={'gibtsnicht' as NavIconName} />);
    expectGlyph(container);
    expect(svgOf(container).getAttribute('data-icon')).toBe('neutral');
  });
});

describe('VIcon', () => {
  it.each(ICONS)('liefert für „%s“ ein eigenes Zeichen', (name) => {
    const { container } = render(<VIcon name={name} />);
    expectGlyph(container);
    expect(svgOf(container).getAttribute('data-icon')).toBe(name);
  });

  it('der Leerzustand ist eine leere Minimap mit „Kein Signal“', () => {
    const { container } = render(<VIcon name="empty" />);
    expect(svgOf(container).classList.contains('gta-v-empty')).toBe(true);
    expect(svgOf(container).textContent).toContain('KEIN SIGNAL');
  });

  it('unbekannte Namen ergeben ein neutrales Zeichen statt eines Fehlers', () => {
    const { container } = render(<VIcon name={'gibtsnicht' as IconName} />);
    expectGlyph(container);
    expect(svgOf(container).getAttribute('data-icon')).toBe('neutral');
  });
});
