import { useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { useApi } from '../api/ApiContext.js';
import { notifyDbActiveChanged, useDbActiveChanged } from '../dbquestion/activeBoard.js';
import { useAsync } from '../hooks/useAsync.js';
import { useLiveHandler } from '../live/useLive.js';
import type { DbActiveDto, DbBoardDto, DbProjectDto } from '../api/dbTypes.js';
import { useBoardProject, type BoardChoice, type ShownBoard } from './boardProject.js';

interface BoardData {
  projects: DbProjectDto[];
  boards: DbBoardDto[];
  active: DbActiveDto;
}

// Angezeigtes Projekt und Board: was die offene Seite meldet, sonst das aktive Board, sonst das erste Projekt
export function currentBoard({ projects, boards, active }: BoardData, shown: ShownBoard | null) {
  const projectKey = shown?.project ?? active.board?.projectKey ?? projects[0]?.key;
  const project = projects.find((p) => p.key === projectKey) ?? projects[0] ?? null;
  const shownBoard = shown && shown.board !== undefined ? shown.board : active.board?.projectKey === project?.key ? active.boardId : null;
  const board = boards.find((b) => b.id === shownBoard && b.projectKey === project?.key) ?? null;
  return { project, board };
}

// Projekte, Boards und aktives Board der Board-Datenbank (lädt nach, wenn sich das aktive Board ändert oder die Verbindung neu
// startet) und die Wahl wie in der Seitenleiste: die offene Seite übernimmt sie, sonst wird das Board aktiv bzw. ein Projekt
// ohne Board auf dem Board geöffnet
export function useBoardChoices(pid: string) {
  const api = useApi();
  const navigate = useNavigate();
  const context = useBoardProject();
  const meta = useAsync(async () => ({ projects: await api.getDbProjects(), boards: await api.getDbBoards(), active: await api.getDbActive() }), [api]);
  const { reload } = meta;
  useDbActiveChanged(() => void reload());
  useLiveHandler({ onReset: () => void reload() });
  const boards = meta.state.status === 'ready' ? meta.state.data.boards : null;
  const choose = useCallback(
    async (choice: BoardChoice) => {
      if (context?.chooser.current) return context.chooser.current(choice);
      const id = choice.board ?? boards?.find((b) => b.projectKey === choice.project)?.id;
      if (id === undefined) return navigate(`/p/${pid}/board`, { state: { project: choice.project } });
      await api.activateDbBoard(id);
      notifyDbActiveChanged();
    },
    [api, boards, context, navigate, pid],
  );
  return { ...meta, choose };
}
