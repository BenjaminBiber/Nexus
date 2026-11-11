import { NavLink } from 'react-router-dom';
import { useNavCounts } from '../hooks/useNavCounts.js';
import { QuestionsIcon } from './NavIcons.js';

// Eintrag „Fragen“ (Board-Datenbank) mit rotem Zähler der offenen eigenen Fragen im Projekt des aktiven Boards (aus
// NavCountsProvider). Ohne offene Fragen, ohne Board-Datenbank oder bei einem Fehler erscheint nur der Name.
export function DbQuestionsNavLink({ pid }: { pid: string }) {
  const open = useNavCounts().questions ?? 0;
  return (
    <NavLink to={`/p/${pid}/questions`} aria-label={open > 0 ? `Fragen, ${open} eigene offen` : undefined}>
      <QuestionsIcon />
      <span className="nav-label">Fragen</span>
      {open > 0 && (
        <span className="nav-badge" aria-hidden="true">
          {open}
        </span>
      )}
    </NavLink>
  );
}
