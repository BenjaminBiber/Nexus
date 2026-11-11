import { useEffect, useRef, useState } from 'react';
import type { DoneGrouping } from '../../api/types.js';

const GROUPINGS: { value: DoneGrouping; label: string }[] = [
  { value: 'time', label: 'Zeit' },
  { value: 'spec', label: 'Spec' },
];

const MENU_LABEL = 'Optionen der Spalte Erledigt';

// Dreipunkt-Menü am Kopf der Spalte „Erledigt“: Gruppierung nach Zeit oder Spec wählen und archivierte Tickets ein- oder ausblenden.
// Der Eintrag für das Archiv erscheint nur, wenn es archivierte Tickets gibt oder sie gerade gezeigt werden.
// Klick außerhalb und Escape schließen das Menü; nach einer Wahl geht der Fokus an den Knopf zurück.
export function DbColumnMenu({ grouping, onGrouping, archived, showArchived, onShowArchived }: {
  grouping: DoneGrouping;
  onGrouping: (grouping: DoneGrouping) => void;
  /** Zahl der gerade ausgeblendeten archivierten Tickets */
  archived: number;
  showArchived: boolean;
  onShowArchived: (show: boolean) => void;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      setOpen(false);
      trigger.current?.focus();
    };
    document.addEventListener('pointerdown', outside);
    document.addEventListener('keydown', escape);
    return () => {
      document.removeEventListener('pointerdown', outside);
      document.removeEventListener('keydown', escape);
    };
  }, [open]);

  // Schließt das Menü, gibt den Fokus an den Knopf zurück und führt die Wahl aus
  const choose = (action: () => void) => {
    trigger.current?.focus();
    setOpen(false);
    action();
  };

  return (
    <div className="db-column-menu" ref={root}>
      <button type="button" ref={trigger} className="db-column-menu-btn" aria-label={MENU_LABEL} aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((value) => !value)}>
        <span className="material-icons" aria-hidden="true">more_horiz</span>
      </button>
      {open && (
        <div className="db-column-menu-list" role="menu" aria-label={MENU_LABEL}>
          <div className="db-column-menu-heading">Gruppieren nach</div>
          {GROUPINGS.map(({ value, label }) => (
            <button key={value} type="button" role="menuitemradio" aria-checked={grouping === value} onClick={() => choose(() => onGrouping(value))}>
              <span className="material-icons" aria-hidden="true">{grouping === value ? 'check' : ''}</span>
              {label}
            </button>
          ))}
          {(archived > 0 || showArchived) && (
            <>
              <div className="db-column-menu-sep" role="separator" />
              <button type="button" role="menuitemcheckbox" aria-checked={showArchived} onClick={() => choose(() => onShowArchived(!showArchived))}>
                <span className="material-icons" aria-hidden="true">{showArchived ? 'check' : ''}</span>
                {showArchived ? 'Archiv anzeigen' : `Archiv anzeigen (${archived})`}
              </button>
            </>
          )}
        </div>
      )}
    </div>
  );
}
