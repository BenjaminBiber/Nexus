import { useContext, useEffect, useRef } from 'react';
import { LiveContext, type LiveHandler } from './liveContext.js';

// Verbindungszustand der Live-Anzeige
export function useLiveConnection() {
  return useContext(LiveContext);
}

// Meldet eine Seite für Live-Meldungen an (immer mit dem neuesten Handler, ohne Neuanmeldung)
export function useLiveHandler(handler: LiveHandler): void {
  const { register } = useContext(LiveContext);
  const latest = useRef(handler);
  latest.current = handler;
  useEffect(
    () =>
      register({
        onNotice: (notice) => latest.current.onNotice?.(notice),
        onSessions: () => latest.current.onSessions?.(),
        onDbBoard: (boardId) => latest.current.onDbBoard?.(boardId),
        onDbQuestions: (projectId) => latest.current.onDbQuestions?.(projectId),
        onDocsChanged: () => latest.current.onDocsChanged?.(),
        onReset: () => latest.current.onReset?.(),
      }),
    [register],
  );
}

// Beobachtet ein DB-Board, solange die Komponente angezeigt wird
export function useDbBoardWatch(boardId: number | null): void {
  const { watchDbBoard } = useContext(LiveContext);
  useEffect(() => {
    watchDbBoard(boardId);
    return () => watchDbBoard(null);
  }, [watchDbBoard, boardId]);
}

// Beobachtet die Fragen eines Board-Projekts, solange die Komponente angezeigt wird
export function useDbProjectWatch(projectId: number | null): void {
  const { watchDbProject } = useContext(LiveContext);
  useEffect(() => (projectId === null ? undefined : watchDbProject(projectId)), [watchDbProject, projectId]);
}
