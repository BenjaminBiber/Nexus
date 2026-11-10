// Attrappe für window.location.reload: jsdom kann nicht navigieren und schreibt sonst nur „Not implemented: navigation“.
// `reload` selbst ist nicht überschreibbar, darum tritt ein eigenes Objekt an die Stelle von window.location.
import { afterEach, beforeEach, vi } from 'vitest';

/** Ersetzt window.location für jeden Test dieser Datei und liefert die Attrappe von reload */
export function stubReload(): { reload: ReturnType<typeof vi.fn> } {
  const stub = { reload: vi.fn() };
  let original: Location;
  beforeEach(() => {
    original = window.location;
    stub.reload = vi.fn();
    // Alles außer reload fragt das echte Location-Objekt (href, search und Co. bleiben damit aktuell)
    const value = { reload: stub.reload, assign: (url: string) => original.assign(url), replace: (url: string) => original.replace(url), toString: () => original.href };
    for (const key of ['href', 'origin', 'protocol', 'host', 'hostname', 'port', 'pathname', 'search', 'hash'] as const) {
      Object.defineProperty(value, key, { enumerable: true, get: () => original[key] });
    }
    Object.defineProperty(window, 'location', { configurable: true, value });
  });
  afterEach(() => {
    Object.defineProperty(window, 'location', { configurable: true, value: original });
  });
  return stub;
}
