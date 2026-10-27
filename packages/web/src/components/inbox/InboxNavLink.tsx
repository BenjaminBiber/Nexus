import { NavLink } from 'react-router-dom';
import { useInbox } from '../../inbox/useInbox.js';
import { InboxIcon } from '../NavIcons.js';

// Eintrag „Meine Aufgaben“ mit Zähler aller wartenden Einträge der eigenen Boards. Ohne Einträge, ohne Board-Datenbank oder
// bei einem Fehler erscheint nur der Name.
export function InboxNavLink({ pid }: { pid: string }) {
  const inbox = useInbox(null);
  const total = inbox.state.status === 'ready' && !inbox.unavailable ? inbox.state.data.total : 0;
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
