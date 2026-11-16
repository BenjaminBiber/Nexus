// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import type { NavIconName } from '@nexus/theme-sdk';
import { AnimatedNavIcon } from '../src/components/navicons/AnimatedNavIcon.js';
import { NAV_ICONS } from '../src/components/navicons/defs.js';
import { BoardIcon, SearchFieldIcon } from '../src/components/NavIcons.js';

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

const NAMES: NavIconName[] = ['board', 'dashboard', 'inbox', 'sessions', 'questions', 'docs', 'search', 'guide', 'settings'];

function inLink(node: React.ReactNode) {
  const { container } = render(<a href="#x">{node}Board</a>);
  return { link: container.querySelector('a')!, svg: container.querySelector('svg')! };
}

const leftColumn = (svg: SVGSVGElement) => svg.querySelector('[data-n="l"]')!.getAttribute('d');

// Bilder von Hand weiterschalten: requestAnimationFrame sammelt, frame(ms) ruft mit Startzeit + ms auf
function manualFrames() {
  let queue: FrameRequestCallback[] = [];
  let t0 = 0;
  vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => {
    if (queue.length === 0) t0 = performance.now();
    queue.push(cb);
    return queue.length;
  });
  vi.stubGlobal('cancelAnimationFrame', () => {
    queue = [];
  });
  return (ms: number) =>
    act(() => {
      const run = queue;
      queue = [];
      run.forEach((cb) => cb(t0 + ms));
    });
}

describe('AnimatedNavIcon', () => {
  it('zeichnet jedes Navigationssymbol als schlichtes Liniensymbol', () => {
    for (const name of NAMES) {
      expect(NAV_ICONS[name]).toBeDefined();
      const { container, unmount } = render(<AnimatedNavIcon name={name} />);
      const svg = container.querySelector('svg')!;
      expect(svg.classList.contains('nav-icon')).toBe(true);
      expect(svg.getAttribute('width')).toBe('18');
      expect(svg.getAttribute('stroke')).toBe('currentColor');
      expect(svg.getAttribute('stroke-width')).toBe('1.75');
      expect(svg.getAttribute('fill')).toBe('none');
      expect(svg.querySelectorAll('[fill]:not(svg), [stroke-width]:not(svg)')).toHaveLength(0);
      unmount();
    }
  });

  it('bewegt das Board-Symbol beim Überfahren des Links und kehrt beim Verlassen zurück', () => {
    vi.stubGlobal('matchMedia', () => ({ matches: true }));
    const { link, svg } = inLink(<BoardIcon />);
    expect(leftColumn(svg)).toBe('M4 4h6v16H4z');
    fireEvent.pointerEnter(link);
    expect(leftColumn(svg)).toBe('M4 4h6v10H4z');
    fireEvent.pointerLeave(link);
    expect(leftColumn(svg)).toBe('M4 4h6v16H4z');
  });

  it('läuft über die Bilder und dreht mitten in der Bewegung weich um', () => {
    const frame = manualFrames();
    const { link, svg } = inLink(<BoardIcon />);
    fireEvent.pointerEnter(link);
    frame(230);
    const mid = leftColumn(svg)!;
    const h = Number(/v([\d.]+)H/.exec(mid)![1]);
    expect(h).toBeGreaterThan(10);
    expect(h).toBeLessThan(16);
    fireEvent.pointerLeave(link);
    frame(0);
    expect(leftColumn(svg)).toBe(mid);
    frame(460);
    expect(leftColumn(svg)).toBe('M4 4h6v16H4z');
  });

  it('reagiert nicht auf Berührungen', () => {
    vi.stubGlobal('matchMedia', () => ({ matches: true }));
    const { link, svg } = inLink(<BoardIcon />);
    const touch = new Event('pointerenter');
    Object.defineProperty(touch, 'pointerType', { value: 'touch' });
    link.dispatchEvent(touch);
    expect(leftColumn(svg)).toBe('M4 4h6v16H4z');
  });

  it('fügt dem Link keinen Text hinzu', () => {
    const { link } = inLink(<BoardIcon />);
    expect(link.textContent).toBe('Board');
  });

  it('lässt die Lupe im Suchfeld still', () => {
    vi.stubGlobal('matchMedia', () => ({ matches: true }));
    const { link, svg } = inLink(<SearchFieldIcon />);
    expect(svg.hasAttribute('data-icon')).toBe(false);
    const before = svg.innerHTML;
    fireEvent.pointerEnter(link);
    expect(svg.innerHTML).toBe(before);
  });
});
