import { createContext, useContext, useEffect, useMemo, useRef, useState, type MutableRefObject, type ReactNode } from 'react';

/** Wahl in der Board-Auswahl der Seitenleiste: ein Board (samt Projekt) oder ein Projekt ohne Board */
export interface BoardChoice {
  project: string;
  board: number | null;
}

/** Was die offene Seite zeigt; board undefined: die Seite zeigt kein bestimmtes Board (Seitenleiste nimmt das aktive) */
export interface ShownBoard {
  project: string;
  board?: number | null;
}

type Chooser = (choice: BoardChoice) => void;

interface BoardProjectState {
  /** Projekt und Board, die die offene Seite gerade zeigt (null: keine Seite meldet eines) */
  shown: ShownBoard | null;
  setShown: (shown: ShownBoard | null) => void;
  /** Wahl der offenen Seite; ohne sie wechselt die Seitenleiste das aktive Board selbst */
  chooser: MutableRefObject<Chooser | null>;
}

const BoardProjectContext = createContext<BoardProjectState | null>(null);

// Verbindet die Board-Auswahl der Seitenleiste mit der offenen Seite
export function BoardProjectProvider({ children }: { children: ReactNode }) {
  const [shown, setShown] = useState<ShownBoard | null>(null);
  const chooser = useRef<Chooser | null>(null);
  const value = useMemo(() => ({ shown, setShown, chooser }), [shown]);
  return <BoardProjectContext.Provider value={value}>{children}</BoardProjectContext.Provider>;
}

export function useBoardProject(): BoardProjectState | null {
  return useContext(BoardProjectContext);
}

// Meldet Projekt (und ggf. Board) der Seite und übernimmt die Wahl in der Seitenleiste, solange die Seite offen ist
export function useBoardProjectSelection(project: string | null, onChoose: Chooser, board?: number | null): void {
  const context = useContext(BoardProjectContext);
  const latest = useRef(onChoose);
  latest.current = onChoose;
  const chooser = context?.chooser;
  const setShown = context?.setShown;
  useEffect(() => {
    if (!chooser || !setShown) return;
    chooser.current = (choice) => latest.current(choice);
    return () => {
      chooser.current = null;
      setShown(null);
    };
  }, [chooser, setShown]);
  useEffect(() => {
    setShown?.(project ? { project, board } : null);
  }, [setShown, project, board]);
}
