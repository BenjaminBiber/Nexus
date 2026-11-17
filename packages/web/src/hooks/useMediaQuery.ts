import { useCallback, useSyncExternalStore } from 'react';

// Großer Bildschirm: breiter als die Laptop-Breite (Gegenstück zu LAPTOP_QUERY in components/sidebarRail.ts)
export const DESKTOP_QUERY = '(min-width: 1601px)';

const mediaList = (query: string) => (typeof window !== 'undefined' && typeof window.matchMedia === 'function' ? window.matchMedia(query) : null);

// Trifft die Medienabfrage zu? Folgt Änderungen der Fenstergröße; ohne matchMedia (z. B. in Tests) nie
export function useMediaQuery(query: string): boolean {
  const subscribe = useCallback(
    (onChange: () => void) => {
      const list = mediaList(query);
      list?.addEventListener?.('change', onChange);
      return () => list?.removeEventListener?.('change', onChange);
    },
    [query],
  );
  return useSyncExternalStore(subscribe, () => mediaList(query)?.matches ?? false, () => false);
}
