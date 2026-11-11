// Warteschlange der Banner (Mission Passed, Busted, Wasted): eins nach dem anderen, jedes BANNER_MS lang.
import { useCallback, useEffect, useMemo, useState } from 'react';
import type { GameEvent } from '../data/events.js';

export const BANNER_MS = 6000;
const MAX_WAITING = 5;

// Eigene Hülle je Eintrag: dasselbe Ereignis zweimal hintereinander ist trotzdem ein neues Banner mit eigener Zeit
interface Entry {
  event: GameEvent;
}

/**
 * Esc schließt das stehende Banner, ohne die Taste abzufangen; ein Esc, das schon ein Dialog des Kerns verarbeitet hat
 * (defaultPrevented), lässt es stehen.
 */
export function useBannerEscape(banner: GameEvent | null, dismiss: () => void): void {
  useEffect(() => {
    if (!banner) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !event.defaultPrevented) dismiss();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [banner, dismiss]);
}

export function useBanners(enabled: boolean): { current: GameEvent | null; push(events: GameEvent[]): void; dismiss(): void } {
  const [queue, setQueue] = useState<Entry[]>([]);
  const head = enabled ? queue[0] : undefined;

  // Abgeschaltet: Wartendes verwerfen, damit nach dem Einschalten nichts Altes kommt
  useEffect(() => {
    if (!enabled) setQueue([]);
  }, [enabled]);

  useEffect(() => {
    if (!head) return;
    const timer = setTimeout(() => setQueue((q) => (q[0] === head ? q.slice(1) : q)), BANNER_MS);
    return () => clearTimeout(timer);
  }, [head]);

  const push = useCallback(
    (events: GameEvent[]) => {
      if (!enabled || events.length === 0) return;
      const entries = events.map((event) => ({ event }));
      setQueue((q) => {
        const [current, ...waiting] = [...q, ...entries];
        return [current!, ...waiting.slice(-MAX_WAITING)];
      });
    },
    [enabled],
  );

  const dismiss = useCallback(() => setQueue((q) => q.slice(1)), []);

  const current = head?.event ?? null;
  return useMemo(() => ({ current, push, dismiss }), [current, push, dismiss]);
}
