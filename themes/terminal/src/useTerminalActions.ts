import { useMemo, useRef } from 'react';
import { useBoards, useColorScheme, useNav, useNavigate, useOpenSearch, useSettings, useUsage } from '@nexus/theme-sdk';
import type { TerminalActions, TerminalBoard } from './terminalCommands.js';

/** Aktionen aus dem Host; den Befehlsverlauf ergänzt die Prompt-Zeile */
export type HostActions = Omit<TerminalActions, 'history'>;

// Pfad unter /p/:pid ohne führenden Schrägstrich, z. B. „board/t/ARC-0001“
export function pathUnderProject(pathname: string, pid: string): string {
  const prefix = `/p/${encodeURIComponent(pid)}/`;
  const alternative = `/p/${pid}/`;
  const rest = pathname.startsWith(prefix) ? pathname.slice(prefix.length) : pathname.startsWith(alternative) ? pathname.slice(alternative.length) : '';
  try {
    return decodeURIComponent(rest.replace(/\/+$/, ''));
  } catch {
    return rest.replace(/\/+$/, '');
  }
}

// Aktueller Pfad unter dem Projekt: Der Bereich kommt aus der Navigation des Hosts (useNav), tiefere Stufen
// (z. B. „board/t/ARC-0001“) aus der Adresse des Browsers, solange sie zum Bereich passt
export function useTerminalPath(pid: string): string {
  const section = useNav().find((item) => item.active)?.id ?? '';
  const address = typeof window === 'undefined' ? '' : pathUnderProject(window.location.pathname, pid);
  if (!section) return address;
  return address === section || address.startsWith(`${section}/`) ? address : section;
}

// Verbindet die Terminal-Befehle mit den Hooks des Theme-SDK: Navigation, Routenwechsel, Suche, Boards, Farbschema und Nutzung
export function useTerminalActions(pid: string): HostActions {
  const nav = useNav();
  const navigate = useNavigate();
  const openSearch = useOpenSearch();
  const boards = useBoards();
  const settings = useSettings();
  const scheme = useColorScheme();
  const usage = useUsage();
  const path = useTerminalPath(pid);

  // Befehle laufen asynchron; sie lesen immer den neuesten Stand statt des Stands beim Abschicken
  const latest = useRef({ nav, boards, settings, scheme, usage, path });
  latest.current = { nav, boards, settings, scheme, usage, path };

  return useMemo<HostActions>(() => {
    const base = `/p/${encodeURIComponent(pid)}`;
    return {
      path: () => latest.current.path,
      sections: () => latest.current.nav.filter((item) => item.id !== 'search').map(({ id, label, badge }) => ({ id, label, badge })),
      go: (to) => navigate(`${base}/${to}`),
      openTicket: (id) => navigate(`${base}/board/t/${encodeURIComponent(id)}`),
      search: () => openSearch(),
      boards: (): TerminalBoard[] => {
        const { boards: list, projects, current } = latest.current.boards;
        return list.map((b) => ({
          id: b.id,
          name: b.name,
          project: projects.find((p) => p.id === b.project)?.name || b.project,
          projectKey: b.project,
          active: current?.board === b.id,
        }));
      },
      switchBoard: (board) => latest.current.boards.choose({ project: board.projectKey, board: board.id }),
      theme: () => latest.current.settings.values?.theme ?? latest.current.scheme.mode ?? null,
      // Terminal bringt Code mit (main.js lädt nur beim Start): Jeder Wechsel weg davon braucht ein Neuladen, sonst bleibt
      // der Terminal-Rahmen bzw. fehlt der Rahmen eines anderen Code-Themes. Das Schema ist nach update() schon gemerkt.
      setTheme: async (mode) => {
        await latest.current.settings.update({ theme: mode });
        if (mode !== 'terminal') window.location.reload();
      },
      usage: () => {
        const { enabled, available, fiveHour, sevenDay } = latest.current.usage;
        return enabled && available ? { fiveHour: fiveHour?.percent ?? null, sevenDay: sevenDay?.percent ?? null } : null;
      },
    };
  }, [navigate, openSearch, pid]);
}
