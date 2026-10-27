import { useEffect, useRef } from 'react';

// Meldung im Browserfenster, wenn das aktive Board gewechselt wurde (der Server kennt dafür keine Live-Meldung)
const ACTIVE_CHANGED = 'harness:db-active-changed';

// Meldet einen Wechsel des aktiven Boards, z. B. nach „Aktivieren“ auf dem Board
export function notifyDbActiveChanged(): void {
  window.dispatchEvent(new Event(ACTIVE_CHANGED));
}

// Ruft `onChange` bei jedem Wechsel des aktiven Boards (immer der neueste Rückruf, ohne Neuanmeldung)
export function useDbActiveChanged(onChange: () => void): void {
  const latest = useRef(onChange);
  latest.current = onChange;
  useEffect(() => {
    const listener = () => latest.current();
    window.addEventListener(ACTIVE_CHANGED, listener);
    return () => window.removeEventListener(ACTIVE_CHANGED, listener);
  }, []);
}

// Meldung im Browserfenster, wenn Projekte oder Boards in der Verwaltung geändert wurden (Anlegen, Umbenennen, Löschen)
const BOARDS_CHANGED = 'harness:db-boards-changed';

export function notifyDbBoardsChanged(): void {
  window.dispatchEvent(new Event(BOARDS_CHANGED));
}

// Ruft `onChange` nach jeder Änderung in der Verwaltung (immer der neueste Rückruf, ohne Neuanmeldung)
export function useDbBoardsChanged(onChange: () => void): void {
  const latest = useRef(onChange);
  latest.current = onChange;
  useEffect(() => {
    const listener = () => latest.current();
    window.addEventListener(BOARDS_CHANGED, listener);
    return () => window.removeEventListener(BOARDS_CHANGED, listener);
  }, []);
}
