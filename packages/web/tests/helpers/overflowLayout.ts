import { vi } from 'vitest';

// Simuliert das Layout einer OverflowTabList in jsdom: jeder Reiter `tab` px, der ⋯-Knopf 40 px, die Leiste `width` px.
// Der ResizeObserver meldet sofort beim Beobachten. Aufräumen mit resetOverflowLayout (afterEach).
export function overflowLayout(width: number, tab = 100) {
  class FakeResizeObserver {
    constructor(private readonly callback: () => void) {}
    observe() { this.callback(); }
    unobserve() {}
    disconnect() {}
  }
  vi.stubGlobal('ResizeObserver', FakeResizeObserver);
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
    const w = this.hasAttribute('data-measure-more') ? 40 : this.hasAttribute('data-measure-tab') ? tab : 0;
    return { width: w, height: 0, x: 0, y: 0, top: 0, left: 0, right: w, bottom: 0, toJSON: () => ({}) } as DOMRect;
  });
  Object.defineProperty(HTMLElement.prototype, 'clientWidth', {
    configurable: true,
    get(this: HTMLElement) { return this.classList.contains('overflow-tabs') ? width : 0; },
  });
}

export function resetOverflowLayout() {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  delete (HTMLElement.prototype as { clientWidth?: number }).clientWidth;
}
