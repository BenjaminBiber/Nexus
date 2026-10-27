import { useEffect, useState, type RefObject } from 'react';

// Merkt sich das beim Öffnen fokussierte Element und gibt ihm den Fokus beim echten Schließen zurück.
// Der Effekt-Probelauf von StrictMode (Aufräumen bei weiter eingehängtem Dialog) stiehlt den Fokus nicht;
// liegt der Fokus nach dem Schließen schon auf einem anderen Element außerhalb, bleibt er dort.
export function useRestoreFocus(container: RefObject<HTMLElement | null>) {
  const [previous] = useState<Element | null>(() => (typeof document === 'undefined' ? null : document.activeElement));
  useEffect(() => {
    const node = container.current;
    return () => {
      if (node?.isConnected) return;
      const active = document.activeElement;
      if (active && active !== document.body && !node?.contains(active)) return;
      if (previous instanceof HTMLElement && previous.isConnected) previous.focus();
    };
  }, [container, previous]);
}
