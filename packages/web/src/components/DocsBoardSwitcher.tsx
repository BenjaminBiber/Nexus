import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import type { DbBoardDto } from '../api/dbTypes.js';

// Anzeigename eines Boards: der Name, bei Namensgleichheit mit dem Projektkürzel davor („AIH Main“, „MDW Main“);
// gilt auch für die Board-Ordner im Explorer
export function boardLabel(boards: DbBoardDto[], board: DbBoardDto): string {
  return boards.filter((b) => b.name === board.name).length > 1 ? `${board.projectKey} ${board.name}` : board.name;
}

// Text des Auslösers: „Alle Boards“, bis zu zwei Namen oder die Anzahl
function summary(boards: DbBoardDto[], shown: number[], all: boolean): string {
  if (all) return 'Alle Boards';
  const names = boards.filter((b) => shown.includes(b.id)).map((b) => boardLabel(boards, b));
  return names.length <= 2 ? names.join(', ') : `${names.length} Boards`;
}

// Board-Umschalter der Docs-Seite als Dropdown mit Mehrfachauswahl: „Alle Boards“ oder beliebig viele einzelne Boards.
// Ein Klick außerhalb oder Escape schließt die Liste. Das letzte gewählte Board lässt sich nicht abwählen – ohne Board gäbe es nichts zu zeigen.
export function DocsBoardSwitcher({ boards, shown, all, onAll, onToggle }: {
  boards: DbBoardDto[];
  /** IDs der Boards, deren Docs gerade zu sehen sind */
  shown: number[];
  all: boolean;
  onAll: () => void;
  onToggle: (id: number) => void;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    const outside = (event: MouseEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', outside);
    return () => document.removeEventListener('mousedown', outside);
  }, [open]);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape' && open) {
      event.preventDefault();
      event.stopPropagation();
      setOpen(false);
      trigger.current?.focus();
    }
  };

  const text = summary(boards, shown, all);
  return (
    <div ref={root} className="docs-boards" onKeyDown={onKeyDown}>
      <button
        ref={trigger}
        type="button"
        className="docs-boards-trigger"
        aria-haspopup="true"
        aria-expanded={open}
        aria-label={`Boards: ${text}`}
        title={text}
        onClick={() => setOpen((o) => !o)}
      >
        <span className="docs-boards-text">{text}</span>
        <svg className={`docs-boards-caret${open ? ' open' : ''}`} width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <path d="M4 6l4 4 4-4" />
        </svg>
      </button>
      {open && (
        <div className="docs-boards-pop" role="group" aria-label="Boards wählen">
          <label className="docs-boards-option is-all">
            <input type="checkbox" checked={all} onChange={onAll} />
            <span>Alle Boards</span>
          </label>
          {boards.map((b) => {
            const on = all || shown.includes(b.id);
            return (
              <label key={b.id} className="docs-boards-option" title={`${b.projectName} › ${b.name}`}>
                <input type="checkbox" checked={on} disabled={!all && on && shown.length === 1} onChange={() => onToggle(b.id)} />
                <span>{boardLabel(boards, b)}</span>
                <span className="docs-boards-project">{b.projectName}</span>
              </label>
            );
          })}
        </div>
      )}
    </div>
  );
}
