import { useEffect, useState } from 'react';
import { NavLink, useLocation } from 'react-router-dom';
import { loadDocsSeen } from '../board/docsSeen.js';
import { useDocList } from '../hooks/useDocs.js';
import { DocsIcon } from './NavIcons.js';

// Eintrag „Docs“ mit Zähler der Dokumente, die seit dem letzten Besuch der Docs-Seite neu oder geändert sind
// (gemerkter Stand je Projekt in localStorage). Live über „docs-changed“ aus `useDocList`; den gemerkten Stand liest
// der Eintrag nach jedem geladenen Stand und bei jedem Adresswechsel neu, damit der Zähler nach dem Lesen auf 0 fällt.
export function DocsNavLink({ pid }: { pid: string }) {
  const docs = useDocList(pid);
  const location = useLocation();
  const [seen, setSeen] = useState(() => loadDocsSeen(pid));
  useEffect(() => setSeen(loadDocsSeen(pid)), [pid, location, docs.state]);
  const list = docs.state.status === 'ready' ? docs.state.data.docs : [];
  const count = list.filter((d) => d.mtimeMs > seen).length;
  return (
    <NavLink to={`/p/${pid}/docs`} aria-label={count > 0 ? `Docs, ${count} neu` : undefined}>
      <DocsIcon />
      <span className="nav-label">Docs</span>
      {count > 0 && (
        <span className="nav-badge" aria-hidden="true">
          {count}
        </span>
      )}
    </NavLink>
  );
}
