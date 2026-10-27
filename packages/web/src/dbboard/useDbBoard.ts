import { useCallback, useRef, useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import { ApiError } from '../api/ApiError.js';
import type { DbBoardViewDto, DbNoteDto } from '../api/dbTypes.js';
import { notifyDbActiveChanged } from '../dbquestion/activeBoard.js';
import { useAsync } from '../hooks/useAsync.js';
import { useDbBoardWatch, useLiveHandler } from '../live/useLive.js';

// Daten des DB-Boards: aktives Board, Projekte, Boards, Ansicht und offene Notizen.
// Eine nicht verfügbare Board-Datenbank (503) steht in `unavailable`; Live-Meldungen laden nach.
export function useDbBoard() {
  const api = useApi();
  const [unavailable, setUnavailable] = useState<string | null>(null);

  // Merkt sich 503-Fehler, damit die Seite den Hinweis auf die Einstellungen zeigen kann
  const guard = <T>(load: () => Promise<T>) => async (): Promise<T> => {
    try {
      return await load();
    } catch (error) {
      if (error instanceof ApiError && error.status === 503) setUnavailable(error.message);
      throw error;
    }
  };

  const active = useAsync(guard(async () => {
    const result = await api.getDbActive();
    setUnavailable(null);
    return result;
  }), [api]);
  const projects = useAsync(guard(() => api.getDbProjects()), [api]);
  const boards = useAsync(guard(() => api.getDbBoards()), [api]);
  const boardId = active.state.status === 'ready' ? active.state.data.boardId : null;
  const boardIdRef = useRef(boardId);
  boardIdRef.current = boardId;
  const view = useAsync<DbBoardViewDto | null>(guard(() => (boardId === null ? Promise.resolve(null) : api.getDbBoardView(boardId))), [api, boardId]);
  const notes = useAsync<DbNoteDto[]>(guard(() => (boardId === null ? Promise.resolve([]) : api.getDbNotes(boardId, 'open'))), [api, boardId]);

  const { reload: reloadActive, set: setActive } = active;
  const { reload: reloadProjects } = projects;
  const { reload: reloadBoards } = boards;
  const { reload: reloadView } = view;
  const { reload: reloadNotes } = notes;

  // Lädt alles neu (nach Verwaltung, „Erneut versuchen“ oder neuer Verbindung). Erst das aktive Board, dann dessen
  // Ansicht: wurde das aktive Board gelöscht oder gewechselt, lädt der Wechsel der Board-ID die neue Ansicht von selbst,
  // das alte Board wird nicht mehr abgefragt.
  const reloadAll = useCallback(() => {
    reloadProjects();
    reloadBoards();
    void reloadActive().then((next) => {
      if (next && next.boardId === boardIdRef.current) {
        reloadView();
        reloadNotes();
      }
    });
  }, [reloadActive, reloadProjects, reloadBoards, reloadView, reloadNotes]);

  useDbBoardWatch(boardId);
  useLiveHandler({
    onDbBoard: (id) => {
      if (id !== boardId) return;
      reloadView();
      reloadNotes();
    },
    onReset: reloadAll,
  });

  // Macht ein Board zum aktiven (board.yaml, gemeinsam mit CLI und Agenten)
  const activate = useCallback(async (id: number) => {
    setActive(await api.activateDbBoard(id));
    notifyDbActiveChanged();
  }, [api, setActive]);

  return { active, projects, boards, view, notes, boardId, unavailable, activate, reloadAll };
}
