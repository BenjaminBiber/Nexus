// Verhalten des Stats-Menüs, gleich in beiden Stilen: Fokus beim Öffnen hinein und danach zurück, Esc schließt, Tab
// bleibt im Dialog, und ein Seitenwechsel (etwa mit Q oder E) schließt das Menü, damit der Inhalt nicht unsichtbar
// weiterläuft.
import { useEffect, useRef, type KeyboardEvent, type RefObject } from 'react';
import { useGame } from '../data/GameContext.js';

const FOCUSABLE = 'button:not([disabled]), [href], [tabindex]:not([tabindex="-1"])';

// Tab bleibt im Dialog: am Ende wieder vorne, am Anfang wieder hinten
function trapTab(event: KeyboardEvent<HTMLElement>): void {
  const items = [...event.currentTarget.querySelectorAll<HTMLElement>(FOCUSABLE)];
  const first = items[0];
  const last = items.at(-1);
  if (!first || !last) return;
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}

/**
 * Für den Dialog des Stats-Menüs: `ref` an den Dialog, `onKeyDown` daran, `close` für Knöpfe. Beim Öffnen bekommt das
 * Element unter `initialFocus` den Fokus, beim Schließen `returnFocus`.
 */
export function useStatsDialog(initialFocus: string, returnFocus?: RefObject<HTMLElement | null>) {
  const { setStatsOpen, active } = useGame();
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    ref.current?.querySelector<HTMLElement>(initialFocus)?.focus();
    const back = returnFocus?.current;
    return () => back?.focus();
  }, [initialFocus, returnFocus]);

  const page = active?.id;
  const opened = useRef(page);
  useEffect(() => {
    if (page !== opened.current) setStatsOpen(false);
  }, [page, setStatsOpen]);

  const close = () => setStatsOpen(false);
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      event.stopPropagation();
      close();
    }
    if (event.key === 'Tab') trapTab(event);
  };
  return { ref, onKeyDown, close };
}
