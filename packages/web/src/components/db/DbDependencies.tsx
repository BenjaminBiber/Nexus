import { useEffect, useRef, useState, type FocusEvent, type KeyboardEvent } from 'react';
import { DB_COLUMN_LABELS, type DbCardDto, type DbColumnId, type DbTicketDetailDto } from '../../api/dbTypes.js';
import { canEditDependencies, pickerGroups, withDependency, withoutDependency } from '../../dbboard/dependencyPicker.js';

// Voraussetzungen und Abhängige eines Tickets als Chips, Auswahl neuer Voraussetzungen im Menü, Hinweis auf Tickets, die mit diesem bereit werden.
export function DbDependencies({ ticket, boardTickets, columns, busy, conflictOpen = false, onChange, onOpen, onShowGraph }: {
  ticket: DbTicketDetailDto;
  boardTickets: DbCardDto[];
  columns: Array<{ id: DbColumnId; title: string }>;
  busy: boolean;
  conflictOpen?: boolean;
  // Die neue Liste wird erst beim Speichern aus dem dann bekannten Stand berechnet, nicht aus dem beim Rendern
  onChange: (update: (current: string[]) => string[]) => Promise<boolean>;
  onOpen: (id: string) => void;
  // Öffnet den Abhängigkeitsgraphen des Tickets (ohne: kein Knopf)
  onShowGraph?: () => void;
}) {
  const [menu, setMenu] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const toggle = useRef<HTMLButtonElement>(null);
  const menuEl = useRef<HTMLDivElement>(null);
  const focusFirst = useRef(false);
  const locked = busy || conflictOpen;
  const editable = canEditDependencies(ticket);
  const open = new Set(ticket.dependsOpen);
  const titleOf = (id: string) => boardTickets.find((c) => c.id === id)?.title;

  // Menüpunkte, die sich anwählen lassen
  const items = () => Array.from(root.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)') ?? []);

  // Nach dem Öffnen per Tastatur auf den ersten Punkt springen
  useEffect(() => {
    if (!menu || !focusFirst.current) return;
    focusFirst.current = false;
    items()[0]?.focus();
  });

  // Klick außerhalb schließt das Menü
  useEffect(() => {
    if (!menu) return;
    const close = (event: MouseEvent) => {
      if (!root.current?.contains(event.target as Node)) setMenu(false);
    };
    document.addEventListener('mousedown', close);
    return () => document.removeEventListener('mousedown', close);
  }, [menu]);

  // Pfeiltasten wechseln zwischen den Punkten; Esc schließt und gibt den Fokus an den Schalter zurück (ohne das Panel zu schließen)
  const onMenuKey = (event: KeyboardEvent) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      setMenu(false);
      toggle.current?.focus();
    } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const list = items();
      if (!list.length) return;
      const at = list.indexOf(document.activeElement as HTMLButtonElement);
      const next = event.key === 'ArrowDown' ? (at + 1) % list.length : at <= 0 ? list.length - 1 : at - 1;
      list[next].focus();
    }
  };

  // Verlässt der Fokus Schalter und Menü (Tab), schließt das Menü
  const onBlur = (event: FocusEvent) => {
    const to = event.relatedTarget as Node | null;
    if (menu && !menuEl.current?.contains(to) && to !== toggle.current) setMenu(false);
  };

  if (!editable && !ticket.dependsOn.length && !ticket.dependents.length) return null;

  // Chip eines Tickets; Klick öffnet es
  const chip = (id: string) => (
    <button
      type="button"
      className={`dep${open.has(id) ? ' is-open' : ''}`}
      title={`${titleOf(id) ? `${id} ${titleOf(id)}` : id} – ${open.has(id) ? 'noch offen' : 'erfüllt'}`}
      onClick={() => onOpen(id)}
    >
      {id}
    </button>
  );

  // Fügt eine Voraussetzung hinzu und schließt das Menü
  const add = async (id: string) => {
    setMenu(false);
    await onChange((current) => withDependency(current, id));
  };

  const groups = menu ? pickerGroups(ticket, boardTickets, columns) : [];
  const gated = ticket.status === 'blocked' ? '„In Arbeit“ und „Erledigt“' : '„Erledigt“';

  return (
    <div className="db-deps" ref={root} onBlur={onBlur}>
      <div className="dep-row" role="group" aria-label="Setzt voraus">
        <span className="dep-label">Setzt voraus</span>
        {ticket.dependsOn.length === 0 && <span className="muted">nichts</span>}
        {ticket.dependsOn.map((id) => (
          <span key={id} className="dep-item">
            {chip(id)}
            {editable && (
              <button type="button" className="tag-remove" aria-label={`${id} entfernen`} disabled={locked} onClick={() => void onChange((current) => withoutDependency(current, id))}>
                ×
              </button>
            )}
          </span>
        ))}
        {editable && (
          <button
            ref={toggle}
            type="button"
            className="tag add"
            aria-haspopup="menu"
            aria-expanded={menu}
            disabled={locked}
            onClick={() => setMenu((m) => !m)}
            onKeyDown={(event) => {
              if (event.key !== 'ArrowDown') return;
              event.preventDefault();
              if (menu) items()[0]?.focus();
              else {
                focusFirst.current = true;
                setMenu(true);
              }
            }}
          >
            + Voraussetzung
          </button>
        )}
      </div>
      {menu && (
        <div ref={menuEl} role="menu" aria-label="Voraussetzung wählen" className="dep-menu" onKeyDown={onMenuKey}>
          {groups.length === 0 && <p className="muted">Keine passenden Tickets auf diesem Board.</p>}
          {groups.map((group) => (
            <div key={group.id} role="group" aria-label={group.title} className="dep-group">
              <span className="dep-group-title">{group.title}</span>
              {group.entries.map((entry) => (
                <button key={entry.id} type="button" role="menuitem" className="dep-option" disabled={entry.disabled || locked} title={entry.reason ?? undefined} onClick={() => void add(entry.id)}>
                  {entry.id} {entry.title}
                  {entry.reason && <span className="muted"> – {entry.reason}</span>}
                </button>
              ))}
            </div>
          ))}
        </div>
      )}
      {ticket.dependents.length > 0 && (
        <div className="dep-row" role="group" aria-label="Voraussetzung für">
          <span className="dep-label">Voraussetzung für</span>
          {ticket.dependents.map((id) => (
            <span key={id} className="dep-item">
              {chip(id)}
            </span>
          ))}
        </div>
      )}
      {ticket.unlocks.length > 0 && (
        <div className="dep-row" role="group" aria-label="Wird bereit, wenn erledigt">
          <span className="dep-label" title="Voraussetzungen erfüllt, sobald dieses Ticket erledigt ist (offene Fragen zählen nicht)">Wird bereit, wenn erledigt</span>
          {ticket.unlocks.map((id) => (
            <span key={id} className="dep-item">
              {chip(id)}
            </span>
          ))}
        </div>
      )}
      {onShowGraph && (ticket.dependsOn.length > 0 || ticket.dependents.length > 0) && (
        <button type="button" className="btn dep-graph-open" onClick={onShowGraph}>
          Graph anzeigen
        </button>
      )}
      {open.size > 0 && (ticket.status === 'in_progress' || ticket.status === 'blocked') && (
        <p className="muted">
          Bleibt in „{DB_COLUMN_LABELS[ticket.status]}“ stehen. Nach {gated} geht es erst, wenn alle Voraussetzungen erfüllt sind.
        </p>
      )}
    </div>
  );
}
