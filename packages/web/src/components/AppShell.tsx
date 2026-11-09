import { useCallback, useEffect, useState, type ReactNode } from 'react';
import { NavLink, Outlet, useParams } from 'react-router-dom';
import { BoardProjectProvider } from '../dbboard/boardProject.js';
import { LiveProvider } from '../live/LiveProvider.js';
import { ConnectionBadge } from './ConnectionBadge.js';
import { ConnectionBanner } from './ConnectionBanner.js';
import { SessionNotifier } from './SessionNotifier.js';
import { SetupPrompts } from './setup/SetupPrompts.js';
import { BoardProjectSwitcher } from './BoardProjectSwitcher.js';
import { DbQuestionsNavLink } from './DbQuestionsNavLink.js';
import { DocsNavLink } from './DocsNavLink.js';
import { DocTicketJobsIndicator } from './DocTicketJobsIndicator.js';
import { BoardIcon, CogIcon, DashboardIcon, GuideIcon, SessionsIcon } from './NavIcons.js';
import { InboxNavLink } from './inbox/InboxNavLink.js';
import { GlobalSearchButton, OpenSearchContext } from './GlobalSearchButton.js';
import { SearchDialog } from './SearchDialog.js';
import { ClaudeUsageMeter } from './ClaudeUsageMeter.js';
import { useDisplayedTheme } from '../theme/useDisplayedTheme.js';
import { DefaultShell } from './DefaultShell.js';
import { SlotBoundary } from '../theme/SlotBoundary.js';
import { useSlot } from '../theme/useSlot.js';
import type { ShellParts } from '@nexus/theme-sdk';

const LINKS: Array<[string, string, () => ReactNode]> = [
  ['board', 'Board', BoardIcon],
  ['dashboard', 'Dashboard', DashboardIcon],
  ['sessions', 'Sessions', SessionsIcon],
  ['guide', 'Anleitung', GuideIcon],
];

// Rahmen aller Seiten: Live-Verbindung des Projekts, Suche (auch Strg+K) und das Layout. Das Layout liefert der Slot
// „Shell“ des angezeigten Themes (z. B. Windows 95 als Desktop), sonst DefaultShell mit Seitenleiste.
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
  const theme = useDisplayedTheme();
  const Shell = useSlot('Shell');
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
  const parts: ShellParts = {
    search: <GlobalSearchButton />,
    boardSwitcher: <BoardProjectSwitcher pid={pid} status={<ConnectionBadge />} />,
    usage: <ClaudeUsageMeter />,
  };
  const standard = <DefaultShell pid={pid} nav={navLinks} content={content} parts={parts} />;

  return (
    <LiveProvider pid={pid} key={pid}>
      <BoardProjectProvider>
        <OpenSearchContext.Provider value={openSearch}>
        <SessionNotifier pid={pid} />
        <SetupPrompts />
        {Shell ? (
          <SlotBoundary fallback={standard} resetKey={theme}>
            <Shell pid={pid} nav={navLinks} content={content} parts={parts} />
          </SlotBoundary>
        ) : (
          standard
        )}
        <DocTicketJobsIndicator />
        {searching && <SearchDialog pid={pid} onClose={() => setSearching(false)} />}
        </OpenSearchContext.Provider>
      </BoardProjectProvider>
    </LiveProvider>
  );
}
