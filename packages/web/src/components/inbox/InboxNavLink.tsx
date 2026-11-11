import { NavLink } from 'react-router-dom';
import { useNavCounts } from '../../hooks/useNavCounts.js';
import { InboxIcon } from '../NavIcons.js';

// Eintrag „Meine Aufgaben“ mit Zähler aller wartenden Einträge der eigenen Boards (aus NavCountsProvider). Ohne Einträge,
// ohne Board-Datenbank oder bei einem Fehler erscheint nur der Name.
export function InboxNavLink({ pid }: { pid: string }) {
  const total = useNavCounts().inbox ?? 0;
  return (
    <NavLink to={`/p/${pid}/inbox`} aria-label={total > 0 ? `Meine Aufgaben, ${total} offen` : undefined}>
      <InboxIcon />
      <span className="nav-label">Meine Aufgaben</span>
      {total > 0 && (
        <span className="nav-badge" aria-hidden="true">
          {total}
        </span>
      )}
    </NavLink>
  );
}
