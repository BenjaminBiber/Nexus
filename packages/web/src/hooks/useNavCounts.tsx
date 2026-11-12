import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
import { useLocation } from 'react-router-dom';
import type { NavIconName } from '@nexus/theme-sdk';
import { useApi } from '../api/ApiContext.js';
import { loadDocsSeen } from '../board/docsSeen.js';
import { useDbActiveChanged } from '../dbquestion/activeBoard.js';
import { useDbQuestions } from '../dbquestion/useDbQuestions.js';
import { useInbox } from '../inbox/useInbox.js';
import { useLiveHandler } from '../live/useLive.js';
import { useAsync } from './useAsync.js';
import { useDocList } from './useDocs.js';

/** Zähler je Eintrag der Hauptnavigation; 0 heißt kein Zähler */
export type NavCounts = Partial<Record<NavIconName, number>>;

// Zahl aller wartenden Einträge der eigenen Boards; ohne Board-Datenbank oder bei einem Fehler 0
function useInboxCount(): number {
  const inbox = useInbox(null);
  return inbox.state.status === 'ready' && !inbox.unavailable ? inbox.state.data.total : 0;
}

// Zahl der offenen eigenen Fragen (Reiter „Eigene“, vom Server gezählt) im Projekt des aktiven Boards (sonst des ersten
// Projekts, wie auf der Fragenseite). Ohne Board-Datenbank (503, nicht eingerichtet) oder bei einem Fehler 0. Live: Meldungen
// zu den Fragen des Projekts, neue Verbindung und Wechsel des aktiven Boards.
function useOwnOpenQuestionsCount(): number {
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

// Zahl der Dokumente, die seit dem letzten Besuch der Docs-Seite neu oder geändert sind (gemerkter Stand je Projekt in
// localStorage). Live über „docs-changed“ aus `useDocList`; den gemerkten Stand liest der Hook nach jedem geladenen Stand und
// bei jedem Adresswechsel neu, damit der Zähler nach dem Lesen auf 0 fällt.
function useNewDocsCount(pid: string): number {
  const docs = useDocList(pid);
  const location = useLocation();
  const [seen, setSeen] = useState(() => loadDocsSeen(pid));
  useEffect(() => setSeen(loadDocsSeen(pid)), [pid, location, docs.state]);
  const list = docs.state.status === 'ready' ? docs.state.data.docs : [];
  return list.filter((d) => d.mtimeMs > seen).length;
}

const NavCountsContext = createContext<NavCounts>({});

// Zählt einmal für Seitenleiste und Themes; braucht Router, ApiProvider und LiveProvider darüber
export function NavCountsProvider({ pid, children }: { pid: string; children: ReactNode }) {
  const inbox = useInboxCount();
  const questions = useOwnOpenQuestionsCount();
  const docs = useNewDocsCount(pid);
  const counts = useMemo<NavCounts>(() => ({ inbox, questions, docs }), [inbox, questions, docs]);
  return <NavCountsContext.Provider value={counts}>{children}</NavCountsContext.Provider>;
}

/** Zähler der Hauptnavigation; ohne NavCountsProvider leer */
export function useNavCounts(): NavCounts {
  return useContext(NavCountsContext);
}
