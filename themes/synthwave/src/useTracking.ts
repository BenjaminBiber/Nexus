import { useEffect, useRef } from 'react';

export const TRACKING_MS = 160;

const ATTRIBUTE = 'data-synth-tracking';

// Kurze Bildstörung beim Wechsel des aktiven Bereichs: data-synth-tracking für TRACKING_MS auf <html>.
// Nicht beim ersten Rendern, nicht ohne VHS und nicht bei reduzierter Bewegung.
export function useTracking(active: string | undefined, enabled: boolean): void {
  const previous = useRef(active);

  useEffect(() => {
    const changed = previous.current !== active;
    previous.current = active;
    if (!changed || !enabled) return;
    if (typeof window.matchMedia === 'function' && window.matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    const root = document.documentElement;
    root.setAttribute(ATTRIBUTE, '');
    const timer = setTimeout(() => root.removeAttribute(ATTRIBUTE), TRACKING_MS);
    return () => {
      clearTimeout(timer);
      root.removeAttribute(ATTRIBUTE);
    };
  }, [active, enabled]);
}
