import { NavLink } from 'react-router-dom';
import { useApi } from '../api/ApiContext.js';
import { useDbActiveChanged } from '../dbquestion/activeBoard.js';
import { useDbQuestions } from '../dbquestion/useDbQuestions.js';
import { useAsync } from '../hooks/useAsync.js';
import { useLiveHandler } from '../live/useLive.js';
import { QuestionsIcon } from './NavIcons.js';

// Zahl der offenen eigenen Fragen (Reiter „Eigene“, vom Server gezählt) im Projekt des aktiven Boards (sonst des ersten
// Projekts, wie auf der Fragenseite). Ohne Board-Datenbank (503, nicht eingerichtet) oder bei einem Fehler 0. Live: Meldungen
// zu den Fragen des Projekts, neue Verbindung und Wechsel des aktiven Boards.
export function useOwnOpenQuestionsCount(): number {
  const api = useApi();
  const meta = useAsync(async () => ({ projects: await api.getDbProjects(), active: await api.getDbActive() }), [api]);
  const { reload } = meta;
  useDbActiveChanged(() => void reload());
  useLiveHandler({ onReset: () => void reload() });
  const projects = meta.state.status === 'ready' ? meta.state.data.projects : [];
  const activeKey = meta.state.status === 'ready' ? meta.state.data.active.board?.projectKey ?? null : null;
  const projectKey = activeKey ?? projects[0]?.key ?? null;
  const projectId = projects.find((p) => p.key === projectKey)?.id ?? null;
  const questions = useDbQuestions(projectKey, projectId);
  const usable = !questions.unavailable && !questions.notice && questions.state.status === 'ready';
  return usable && questions.state.status === 'ready' ? questions.state.data?.counts.mine ?? 0 : 0;
}

// Eintrag „Fragen“ (Board-Datenbank) mit rotem Zähler der offenen eigenen Fragen; ohne offene Fragen erscheint nur der Name
export function DbQuestionsNavLink({ pid }: { pid: string }) {
  const open = useOwnOpenQuestionsCount();
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
