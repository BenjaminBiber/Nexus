// @vitest-environment jsdom
// Symbole im Stil San Andreas für die Slots NavIcon und Icon: flächige Zeichen in currentColor, für Screenreader ausgeblendet
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';
import type { IconName, NavIconName } from '@nexus/theme-sdk';
import { SaIcon } from '../src/sa/SaIcon.js';
import { SaNavIcon } from '../src/sa/SaNavIcon.js';
import { VNavIcon } from '../src/v/VNavIcon.js';

afterEach(cleanup);

const NAV: NavIconName[] = ['board', 'dashboard', 'inbox', 'sessions', 'questions', 'docs', 'search', 'guide', 'settings'];
const ICONS: IconName[] = ['doc', 'plan', 'glossary', 'adr', 'file', 'folder', 'folderOpen', 'empty', 'edit', 'comment'];

const svgOf = (container: HTMLElement) => container.querySelector('svg') as SVGSVGElement;
const markup = (ui: React.ReactElement) => {
  const { container } = render(ui);
  const shape = svgOf(container).innerHTML;
  cleanup();
  return shape;
};

function expectGlyph(container: HTMLElement) {
  const svg = svgOf(container);
  expect(svg).not.toBeNull();
  expect(svg.getAttribute('aria-hidden')).toBe('true');
  expect(svg.getAttribute('focusable')).toBe('false');
  expect(svg.getAttribute('fill')).toBe('currentColor');
  expect(svg.classList.contains('gta-sa-icon')).toBe(true);
  expect(svg.querySelectorAll('path, rect, circle').length).toBeGreaterThan(0);
}

describe('SaNavIcon', () => {
  it.each(NAV)('liefert für „%s“ ein eigenes Zeichen', (name) => {
    const { container } = render(<SaNavIcon name={name} />);
    expectGlyph(container);
    expect(svgOf(container).getAttribute('data-icon')).toBe(name);
  });

  it('jedes Navigationssymbol sieht anders aus – und anders als im Stil GTA V', () => {
    const shapes = NAV.map((name) => markup(<SaNavIcon name={name} />));
    expect(new Set(shapes).size).toBe(NAV.length);
    const v = NAV.map((name) => markup(<VNavIcon name={name} />));
    for (const [i, shape] of shapes.entries()) expect(shape, NAV[i]).not.toBe(v[i]);
  });

  it('unbekannte Namen ergeben ein neutrales Zeichen statt eines Fehlers', () => {
    const { container } = render(<SaNavIcon name={'gibtsnicht' as NavIconName} />);
    expectGlyph(container);
    expect(svgOf(container).getAttribute('data-icon')).toBe('neutral');
  });
});

describe('SaIcon', () => {
  it.each(ICONS)('liefert für „%s“ ein eigenes Zeichen', (name) => {
    const { container } = render(<SaIcon name={name} />);
    expectGlyph(container);
    expect(svgOf(container).getAttribute('data-icon')).toBe(name);
  });

  it('jedes Symbol sieht anders aus', () => {
    expect(new Set(ICONS.map((name) => markup(<SaIcon name={name} />))).size).toBe(ICONS.length);
  });

  it('der Leerzustand ist ein rundes Radar ohne Blips mit „Kein Signal“', () => {
    const { container } = render(<SaIcon name="empty" />);
    expect(svgOf(container).classList.contains('gta-sa-empty')).toBe(true);
    expect(svgOf(container).querySelector('circle')).not.toBeNull();
    expect(svgOf(container).textContent).toContain('KEIN SIGNAL');
  });

  it('unbekannte Namen ergeben ein neutrales Zeichen statt eines Fehlers', () => {
    const { container } = render(<SaIcon name={'gibtsnicht' as IconName} />);
    expectGlyph(container);
    expect(svgOf(container).getAttribute('data-icon')).toBe('neutral');
  });
});
