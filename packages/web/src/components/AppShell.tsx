import { useCallback, useEffect, useState, type ReactNode } from 'react';
import { NavLink, Outlet, useParams } from 'react-router-dom';
import { BoardProjectProvider } from '../dbboard/boardProject.js';
import { LiveProvider } from '../live/LiveProvider.js';
import { ConnectionBadge } from './ConnectionBadge.js';
import { ConnectionBanner } from './ConnectionBanner.js';
import { SessionNotifier } from './SessionNotifier.js';
import { BoardProjectSwitcher } from './BoardProjectSwitcher.js';
import { DbQuestionsNavLink } from './DbQuestionsNavLink.js';
import { DocsNavLink } from './DocsNavLink.js';
import { DocTicketJobsIndicator } from './DocTicketJobsIndicator.js';
import { BoardIcon, CogIcon, DashboardIcon, GuideIcon, SessionsIcon } from './NavIcons.js';
import { InboxNavLink } from './inbox/InboxNavLink.js';
import { GlobalSearchButton, OpenSearchContext } from './GlobalSearchButton.js';
import { SearchDialog } from './SearchDialog.js';
import { ClaudeUsageMeter } from './ClaudeUsageMeter.js';
import { GooNav } from './goo/GooNav.js';
import { useDisplayedTheme } from '../theme/useDisplayedTheme.js';
import { Win95Frame } from './win95/Win95Frame.js';

const LINKS: Array<[string, string, () => ReactNode]> = [
  ['board', 'Board', BoardIcon],
  ['dashboard', 'Dashboard', DashboardIcon],
  ['sessions', 'Sessions', SessionsIcon],
  ['guide', 'Anleitung', GuideIcon],
];

// Rahmen aller Seiten: Seitenleiste mit globaler Suche (auch Strg+K) ganz oben, darunter Board-Auswahl (mit der Verbindung
// als Punkt), dann die Navigation (zuletzt die Einstellungen) und ganz unten die Claude-Nutzung; Live-Verbindung des Projekts.
// Im Schema „Windows 95“ dieselben Teile als Desktop mit Fenster, Startmenü und Taskleiste (Win95Frame).
export function AppShell() {
  const { pid = '' } = useParams();
  const [searching, setSearching] = useState(false);
  // Strg+K bzw. Cmd+K öffnet die Suche über alle Boards von überall
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        setSearching(true);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);
  const openSearch = useCallback(() => setSearching(true), []);
  // Im Schema „Windows 95“ ordnet sich der Rahmen wie der Desktop an (Symbole, Fenster, Taskleiste)
  const theme = useDisplayedTheme();
  const link = ([path, label, Icon]: (typeof LINKS)[number]) => (
    <NavLink key={path} to={`/p/${pid}/${path}`}>
      <Icon />
      <span className="nav-label">{label}</span>
    </NavLink>
  );
  const navLinks = (
    <>
      {LINKS.slice(0, 1).map(link)}
      <InboxNavLink pid={pid} />
      {LINKS.slice(1, 3).map(link)}
      <DbQuestionsNavLink pid={pid} />
      <DocsNavLink pid={pid} />
      {LINKS.slice(3).map(link)}
      <NavLink to={`/p/${pid}/settings`}>
        <CogIcon />
        <span className="nav-label">Einstellungen</span>
      </NavLink>
    </>
  );
  const content = (
    <div className="shell-content">
      <main className="shell-main">
        <div className="page connection-slot">
          <ConnectionBanner />
        </div>
        <Outlet />
      </main>
    </div>
  );

  return (
    <LiveProvider pid={pid} key={pid}>
      <BoardProjectProvider>
        <OpenSearchContext.Provider value={openSearch}>
        <SessionNotifier pid={pid} />
        {theme === 'win95' ? (
          <Win95Frame pid={pid} nav={navLinks} content={content} />
        ) : (
          <>
            <aside className="shell-sidebar">
              <div className="shell-dock">
                <GlobalSearchButton />
                <div className="dock-row">
                  <BoardProjectSwitcher pid={pid} status={<ConnectionBadge />} />
                </div>
              </div>
              <GooNav className="nav" aria-label="Hauptnavigation">
                {navLinks}
              </GooNav>
              <div className="usage-dock">
                <ClaudeUsageMeter />
              </div>
            </aside>
            {content}
          </>
        )}
        <DocTicketJobsIndicator />
        {searching && <SearchDialog pid={pid} onClose={() => setSearching(false)} />}
        </OpenSearchContext.Provider>
      </BoardProjectProvider>
    </LiveProvider>
  );
}
