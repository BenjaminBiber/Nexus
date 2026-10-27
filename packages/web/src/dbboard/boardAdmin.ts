import { useEffect, useRef } from 'react';

// Meldung im Browserfenster, wenn die Board-Verwaltung geöffnet werden soll (sie hängt an der Board-Auswahl im Rahmen)
const ADMIN_OPEN = 'harness:db-admin-open';

// Bittet darum, den Dialog „Board bearbeiten“ zu öffnen, z. B. aus dem Hinweis auf der Docs-Seite
export function notifyOpenBoardAdmin(): void {
  window.dispatchEvent(new Event(ADMIN_OPEN));
}

// Ruft `onOpen`, wenn die Board-Verwaltung geöffnet werden soll (immer der neueste Rückruf, ohne Neuanmeldung)
export function useOpenBoardAdmin(onOpen: () => void): void {
  const latest = useRef(onOpen);
  latest.current = onOpen;
  useEffect(() => {
    const listener = () => latest.current();
    window.addEventListener(ADMIN_OPEN, listener);
    return () => window.removeEventListener(ADMIN_OPEN, listener);
  }, []);
}
