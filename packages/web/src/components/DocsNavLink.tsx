import { NavLink } from 'react-router-dom';
import { useNavCounts } from '../hooks/useNavCounts.js';
import { DocsIcon } from './NavIcons.js';

// Eintrag „Docs“ mit Zähler der Dokumente, die seit dem letzten Besuch der Docs-Seite neu oder geändert sind (aus
// NavCountsProvider)
export function DocsNavLink({ pid }: { pid: string }) {
  const count = useNavCounts().docs ?? 0;
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
