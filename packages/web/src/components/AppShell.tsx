import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react';
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
import { NexusThemeHost } from '../theme/ThemeHost.js';
import { navLabel } from '../theme/navItems.js';
import type { ShellParts } from '@nexus/theme-sdk';

const LINKS: Array<[string, string, () => ReactNode]> = [
  ['board', navLabel('board'), BoardIcon],
  ['dashboard', navLabel('dashboard'), DashboardIcon],
  ['sessions', navLabel('sessions'), SessionsIcon],
  ['guide', navLabel('guide'), GuideIcon],
];

// Rahmen aller Seiten: Live-Verbindung des Projekts, Suche (auch Strg+K) und das Layout. Das Layout liefert der Slot
// „Shell“ des angezeigten Themes (z. B. ein Desktop mit Fenstern), sonst DefaultShell mit Seitenleiste.
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
  // Bausteine des Rahmens; einzeln für Themes und gemeinsam für den Host der SDK-Bausteine
  const parts = useMemo<ShellParts>(() => {
    const link = ([path, label, Icon]: (typeof LINKS)[number]) => (
      <NavLink key={path} to={`/p/${pid}/${path}`}>
        <Icon />
        <span className="nav-label">{label}</span>
      </NavLink>
    );
    const [board, dashboard, sessions, guide] = LINKS.map(link);
    return {
      search: <GlobalSearchButton />,
      boardSwitcher: <BoardProjectSwitcher pid={pid} status={<ConnectionBadge />} />,
      usage: <ClaudeUsageMeter />,
      navItems: {
        board,
        inbox: <InboxNavLink key="inbox" pid={pid} />,
        dashboard,
        sessions,
        questions: <DbQuestionsNavLink key="questions" pid={pid} />,
        docs: <DocsNavLink key="docs" pid={pid} />,
        guide,
        settings: (
          <NavLink key="settings" to={`/p/${pid}/settings`}>
            <CogIcon />
            <span className="nav-label">{navLabel('settings')}</span>
          </NavLink>
        ),
      },
      outlet: <Outlet />,
      connection: <ConnectionBanner />,
      status: <ConnectionBadge />,
    };
  }, [pid]);
  const { navItems } = parts;
  const navLinks = (
    <>
      {navItems.board}
      {navItems.inbox}
      {navItems.dashboard}
      {navItems.sessions}
      {navItems.questions}
      {navItems.docs}
      {navItems.guide}
      {navItems.settings}
    </>
  );
  const content = (
    <div className="shell-content">
      <main className="shell-main">
        <div className="page connection-slot">{parts.connection}</div>
        {parts.outlet}
      </main>
    </div>
  );
  const standard = <DefaultShell pid={pid} nav={navLinks} content={content} parts={parts} />;

  return (
    <LiveProvider pid={pid} key={pid}>
      <BoardProjectProvider>
        <NexusThemeHost pid={pid} parts={parts} openSearch={openSearch}>
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
        </NexusThemeHost>
      </BoardProjectProvider>
    </LiveProvider>
  );
}
