import { createContext, useContext } from 'react';
import { SearchIcon } from './NavIcons.js';

// Der Rahmen (AppShell) stellt das Öffnen des Such-Dialogs bereit; außerhalb des Rahmens bleibt der Knopf wirkungslos.
export const OpenSearchContext = createContext<() => void>(() => {});

// Knopf der Suche über alle Boards (auch Strg+K), unten im Block der Seitenleiste.
export function GlobalSearchButton() {
  const open = useContext(OpenSearchContext);
  return (
    <button
      type="button"
      className="global-search"
      onClick={open}
      aria-keyshortcuts="Control+K Meta+K"
      title="Über alle Boards suchen: Tickets, Fragen, Notizen"
    >
      <SearchIcon />
      <span className="global-search-label">Suchen</span>
      <kbd className="nav-kbd" aria-hidden="true">Strg K</kbd>
    </button>
  );
}
