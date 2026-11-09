import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type ReactNode } from 'react';
import { useOpenBoardAdmin } from '../dbboard/boardAdmin.js';
import { useBoardProject, type BoardChoice } from '../dbboard/boardProject.js';
import { currentBoard, useBoardChoices } from '../dbboard/useBoardChoices.js';
import { notifyDbBoardsChanged } from '../dbquestion/activeBoard.js';
import { DbAdminDialog } from './db/DbAdminDialog.js';
import { prefersReducedMotion } from './goo/goo.js';
import { GooMorph } from './goo/GooMorph.js';

const POPOVER_WIDTH = 480;
// Spätestens danach verschwindet die Goo-Ebene von Aufgang und Abgang (ms), auch wenn ihre Animation nicht zu Ende läuft
const MORPH_MS = 1500;
// Maße der Board-Liste im Popover (px), passend zu .bp-board / .bp-list in base.css
const BOARD_ROW = 50;
const BOARD_GAP = 2;
const BOARD_LIST_MAX = 360;

// Board-Auswahl der Seitenleiste als Pfad „Projekt › Board“. Das Popover zeigt links die Projekte, rechts deren Boards, oben
// eine Suche über beide. Angezeigt wird, was die offene Seite meldet (Board, Fragen, Dashboard), sonst das aktive Board.
// Die Wahl übernimmt die offene Seite; auf anderen Seiten wird das gewählte Board aktiv, ein Projekt ohne Board öffnet das
// Board mit diesem Projekt. Der Stift rechts daneben öffnet die Verwaltung der Projekte und Boards (auch ohne Projekt, damit
// sich das erste anlegen lässt). Links steht als Punkt `status` (die Verbindungsanzeige, deren Text beim Überfahren
// erscheint). Ohne Board-Datenbank erscheint nur `status`.
export function BoardProjectSwitcher({ pid, status }: { pid: string; status?: ReactNode }) {
  const context = useBoardProject();
  const meta = useBoardChoices(pid);
  const { reload, refresh, choose } = meta;
  const [open, setOpen] = useState(false);
  // Das Popover bleibt beim Schließen stehen, bis sein Goo-Abgang gelaufen ist
  const [leaving, setLeaving] = useState(false);
  const leaveTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(leaveTimer.current), []);
  const trigger = useRef<HTMLButtonElement>(null);
  const field = useRef<HTMLDivElement>(null);
  // Zeigt die Seite ein anderes Board (z. B. nach Löschen in der Verwaltung) oder öffnet das Popover: Liste im Hintergrund auffrischen
  const shownKey = context?.shown ? `${context.shown.project}|${context.shown.board}` : '';
  const first = useRef(true);
  useEffect(() => {
    if (first.current) first.current = false;
    else void refresh();
  }, [shownKey, refresh]);
  useEffect(() => {
    if (open) void refresh();
  }, [open, refresh]);
  const [admin, setAdmin] = useState(false);
  // Andere Stellen (etwa der Hinweis auf der Docs-Seite) öffnen die Board-Verwaltung über eine Fenster-Meldung
  useOpenBoardAdmin(() => setAdmin(true));
  // Beim ersten Laden steht schon das spätere Layout (Platzhalter), damit nichts nachrückt
  if (meta.state.status === 'loading') {
    return (
      <div className="proj board-picker" aria-hidden="true">
        <div className="bp-field">
          <span className="board-picker-trigger bp-skel-text">
            <span className="bp-skel-line short" />
            <span className="bp-skel-line" />
          </span>
          <span className="bp-edit" />
        </div>
      </div>
    );
  }
  // Ohne Board-Datenbank bleibt nur die Verbindungsanzeige stehen
  if (meta.state.status !== 'ready') return status ? <div className="bp-status-only">{status}</div> : null;
  const { projects, boards } = meta.state.data;

  const { project, board } = currentBoard(meta.state.data, context?.shown ?? null);

  const closed = () => {
    clearTimeout(leaveTimer.current);
    setLeaving(false);
    setOpen(false);
  };
  const close = () => {
    trigger.current?.focus();
    if (!open || leaving) return;
    if (prefersReducedMotion()) return closed();
    setLeaving(true);
    leaveTimer.current = setTimeout(closed, MORPH_MS);
  };

  const label = project ? `${project.name}${board ? ` › ${board.name}` : ''}` : 'Noch kein Projekt';
  return (
    <div className="proj board-picker">
      {/* Eine Zeile: Verbindungspunkt, die Auswahl (Projekt über Board) und der Stift zur Verwaltung */}
      <div ref={field} className="bp-field">
        {status && <span className="bp-status">{status}</span>}
        <button
          ref={trigger}
          type="button"
          className="board-picker-trigger"
          aria-haspopup="dialog"
          aria-expanded={open && !leaving}
          aria-label={`Board wählen: ${label}`}
          title={label}
          disabled={!project}
          onClick={() => (open ? close() : setOpen(true))}
        >
          {project ? (
            <span className="bp-text">
              <span className="bp-project">{project.name}</span>
              <span className={board ? 'bp-name' : 'bp-name bp-nobody'}>{board ? board.name : 'Kein Board gewählt'}</span>
            </span>
          ) : (
            <span className="bp-name bp-project">{label}</span>
          )}
        </button>
        <button
          type="button"
          className="bp-edit"
          aria-label="Projekte und Boards verwalten"
          title="Projekte und Boards verwalten"
          onClick={() => {
            closed();
            setAdmin(true);
          }}
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M12 20h9" />
            <path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z" />
          </svg>
        </button>
      </div>
      {admin && (
        <DbAdminDialog
          projects={projects}
          boards={boards}
          onClose={() => setAdmin(false)}
          onChanged={() => {
            void reload();
            notifyDbBoardsChanged();
          }}
        />
      )}
      {open && project && (
        <BoardPickerPopover
          anchor={field.current}
          projects={projects}
          boards={boards}
          projectKey={project.key}
          boardId={board?.id ?? null}
          closing={leaving}
          onClose={close}
          onClosed={closed}
          onChoose={(choice) => {
            close();
            void choose(choice).catch(() => reload());
          }}
        />
      )}
    </div>
  );
}

interface PickerProject {
  key: string;
  name: string;
}

interface PickerBoard {
  id: number;
  name: string;
  projectKey: string;
  ownerName: string;
  openTickets: number;
}

interface PopoverProps {
  anchor: HTMLElement | null;
  projects: PickerProject[];
  boards: PickerBoard[];
  projectKey: string;
  boardId: number | null;
  closing: boolean;
  onClose: () => void;
  onClosed: () => void;
  onChoose: (choice: BoardChoice) => void;
}

// Lage des Popovers am Feld, im Fenster gehalten (16 px Rand): unter dem Feld, oder darüber, wenn oben mehr Platz ist;
// die Höhe ist auf den Platz begrenzt, der Inhalt scrollt dann
function placement(anchor: HTMLElement | null): CSSProperties {
  const rect = anchor?.getBoundingClientRect();
  const width = Math.min(POPOVER_WIDTH, window.innerWidth - 32);
  const left = Math.max(16, Math.min(rect?.left ?? 16, window.innerWidth - width - 16));
  const below = window.innerHeight - (rect?.bottom ?? 0) - 6 - 16;
  const above = (rect?.top ?? 0) - 6 - 16;
  if (above > below) return { position: 'fixed', bottom: window.innerHeight - (rect?.top ?? 0) + 6, left, width, maxHeight: above, overflowY: 'auto' };
  return { position: 'fixed', top: (rect?.bottom ?? 0) + 6, left, width, maxHeight: below, overflowY: 'auto' };
}

// Fokus innerhalb einer Liste von Knöpfen mit Pfeiltasten verschieben
function moveFocus(list: HTMLElement | null, step: 1 | -1 | 'first') {
  const items = Array.from(list?.querySelectorAll<HTMLButtonElement>('button') ?? []);
  if (!items.length) return;
  if (step === 'first') return items[0].focus();
  const at = items.indexOf(document.activeElement as HTMLButtonElement);
  items[(at + step + items.length) % items.length].focus();
}

function BoardPickerPopover({ anchor, projects, boards, projectKey, boardId, closing, onClose, onClosed, onChoose }: PopoverProps) {
  const [query, setQuery] = useState('');
  const [preview, setPreview] = useState(projectKey);
  const [style, setStyle] = useState<CSSProperties>(() => placement(anchor));
  const root = useRef<HTMLDivElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const projectList = useRef<HTMLUListElement>(null);
  const boardList = useRef<HTMLUListElement>(null);
  // Goo-Aufgang: vom Feld auf die Größe des Popovers. Gemessen wird vor dem ersten Zeichnen, die Ebene läuft nur kurz.
  // Beim Schließen läuft sie rückwärts und löst dann das eigentliche Schließen aus.
  const [morph, setMorph] = useState<{ from: DOMRect; to: DOMRect; closing: boolean } | null>(null);
  useLayoutEffect(() => {
    const to = root.current?.getBoundingClientRect();
    if (!anchor || !to || prefersReducedMotion()) return;
    setMorph({ from: anchor.getBoundingClientRect(), to, closing: false });
    const timer = setTimeout(() => setMorph((m) => (m?.closing ? m : null)), MORPH_MS);
    return () => clearTimeout(timer);
  }, [anchor]);
  useLayoutEffect(() => {
    if (!closing) return;
    const to = root.current?.getBoundingClientRect();
    if (!anchor || !to) return onClosed();
    setMorph({ from: anchor.getBoundingClientRect(), to, closing: true });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [closing]);

  // Fest positioniert, damit die schmale (scrollende) Seitenleiste das Popover nicht abschneidet. Die Lage steht schon beim
  // ersten Rendern fest: ein anfangs verstecktes Popover nähme den Fokus für die Suche nicht an.
  useEffect(() => {
    const place = () => setStyle(placement(anchor));
    window.addEventListener('resize', place);
    return () => window.removeEventListener('resize', place);
  }, [anchor]);

  // Beim Öffnen in die Suche; ein Klick außerhalb (nicht auf den Auslöser) schließt
  const latestClose = useRef(onClose);
  latestClose.current = onClose;
  useEffect(() => {
    search.current?.focus();
    const outside = (event: MouseEvent) => {
      const target = event.target as Node;
      if (!root.current?.contains(target) && !anchor?.contains(target)) latestClose.current();
    };
    document.addEventListener('mousedown', outside);
    return () => document.removeEventListener('mousedown', outside);
  }, [anchor]);

  const needle = query.trim().toLowerCase();
  const matches = (text: string) => text.toLowerCase().includes(needle);
  const projectMatches = (p: PickerProject) => !needle || matches(p.key) || matches(p.name);
  const boardsOf = (key: string) => {
    const own = boards.filter((b) => b.projectKey === key);
    const p = projects.find((x) => x.key === key);
    return !needle || (p && projectMatches(p)) ? own : own.filter((b) => matches(b.name) || matches(b.ownerName));
  };
  const visibleProjects = projects.filter((p) => projectMatches(p) || boardsOf(p.key).length > 0);
  const current = visibleProjects.some((p) => p.key === preview) ? preview : visibleProjects[0]?.key ?? null;
  const currentProject = projects.find((p) => p.key === current) ?? null;
  const currentBoards = current ? boardsOf(current) : [];
  // Die Board-Spalte behält die Höhe des Projekts mit den meisten Boards, damit der Dialog beim Überfahren nicht springt
  const boardRows = Math.max(1, ...visibleProjects.map((p) => boardsOf(p.key).length));
  const boardsHeight = Math.min(BOARD_LIST_MAX, boardRows * (BOARD_ROW + BOARD_GAP) - BOARD_GAP);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      onClose();
    }
  };

  const onSearchKey = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      moveFocus(projectList.current, 'first');
    } else if (event.key === 'Enter' && current) {
      event.preventDefault();
      onChoose({ project: current, board: currentBoards[0]?.id ?? null });
    }
  };

  const onProjectKey = (event: KeyboardEvent<HTMLUListElement>) => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      moveFocus(projectList.current, event.key === 'ArrowDown' ? 1 : -1);
    } else if (event.key === 'ArrowRight') {
      event.preventDefault();
      moveFocus(boardList.current, 'first');
    }
  };

  const onBoardKey = (event: KeyboardEvent<HTMLUListElement>) => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      moveFocus(boardList.current, event.key === 'ArrowDown' ? 1 : -1);
    } else if (event.key === 'ArrowLeft') {
      event.preventDefault();
      projectList.current?.querySelector<HTMLButtonElement>('[aria-current="true"]')?.focus();
    }
  };

  return (
    <>
    {morph && <GooMorph key={morph.closing ? 'out' : 'in'} from={morph.from} to={morph.to} closing={morph.closing} onDone={() => (morph.closing ? onClosed() : setMorph(null))} />}
    <div ref={root} className={`board-picker-pop${closing ? ' is-closing' : ''}`} role="dialog" aria-label="Board wählen" style={style} onKeyDown={onKeyDown}>
      <input
        ref={search}
        type="search"
        className="bp-search"
        aria-label="Projekt oder Board suchen"
        placeholder="Projekt oder Board suchen …"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={onSearchKey}
      />
      {visibleProjects.length === 0 ? (
        <p className="bp-empty muted">Nichts gefunden.</p>
      ) : (
        <div className="bp-cols">
          <div className="bp-col">
            <span className="bp-col-title" id="bp-projects">Projekte</span>
            <ul ref={projectList} className="bp-list" aria-labelledby="bp-projects" onKeyDown={onProjectKey}>
              {visibleProjects.map((p) => {
                const count = boards.filter((b) => b.projectKey === p.key).length;
                return (
                  <li key={p.key}>
                    <button
                      type="button"
                      className="bp-item"
                      aria-current={p.key === current ? 'true' : undefined}
                      tabIndex={p.key === current ? 0 : -1}
                      onFocus={() => setPreview(p.key)}
                      onMouseEnter={() => setPreview(p.key)}
                      onClick={() => onChoose({ project: p.key, board: boards.find((b) => b.projectKey === p.key)?.id ?? null })}
                    >
                      <span className="bp-key">{p.key}</span>
                      <span className="bp-item-name">{p.name}</span>
                      <span className="bp-count" aria-hidden="true">{count}</span>
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>
          <div className="bp-col">
            <span className="bp-col-title" id="bp-boards">
              Boards{currentProject ? ` in ${currentProject.key}` : ''}
            </span>
            {currentBoards.length > 0 ? (
              <ul ref={boardList} className="bp-list" aria-labelledby="bp-boards" style={{ minHeight: boardsHeight }} onKeyDown={onBoardKey}>
                {currentBoards.map((b, i) => (
                  <li key={b.id}>
                    <button
                      type="button"
                      className="bp-item bp-board"
                      aria-current={b.id === boardId ? 'true' : undefined}
                      tabIndex={i === 0 ? 0 : -1}
                      onClick={() => onChoose({ project: b.projectKey, board: b.id })}
                    >
                      <span className="bp-item-name">{b.name}</span>
                      <span className="bp-meta">
                        {b.ownerName} · {b.openTickets} offen
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            ) : (
              current && (
                <div className="bp-none" style={{ minHeight: boardsHeight }}>
                  <span className="muted">Noch kein Board.</span>
                  <button type="button" className="btn" onClick={() => onChoose({ project: current, board: null })}>
                    Projekt öffnen
                  </button>
                </div>
              )
            )}
          </div>
        </div>
      )}
    </div>
    </>
  );
}
