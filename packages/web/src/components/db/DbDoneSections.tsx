import { useState } from 'react';
import type { DispatchMergeDto } from '../../api/dispatchTypes.js';
import type { DoneLayout } from '../../dbboard/doneSections.js';
import { DbDoneRow } from './DbDoneRow.js';

// Inhalt der Spalte „Erledigt“: Abschnitte mit einklappbarem Kopf und einer Zeile je Ticket, darunter die Zahl der archivierten Tickets.
// Der Nutzer klappt Abschnitte selbst auf und zu; mit expandAll (aktive Suche) sind alle offen und die Köpfe schalten nicht um.
export function DbDoneSections({ layout, expandAll, onOpen, merges, now }: {
  layout: DoneLayout;
  expandAll: boolean;
  onOpen: (id: string) => void;
  /** Laufende Merges des Dispatchers: Ticket-ID → Phase */
  merges?: ReadonlyMap<string, DispatchMergeDto['phase']>;
  now: Date;
}) {
  // Vom Nutzer umgeschaltete Abschnitte (Schlüssel → eingeklappt); ohne Eintrag gilt der Ausgangswert des Abschnitts
  const [toggled, setToggled] = useState<Record<string, boolean>>({});
  return (
    <>
      {layout.shown === 0 && <div className="column-empty">Keine Tickets</div>}
      {layout.sections.map((section) => {
        const collapsed = !expandAll && (toggled[section.key] ?? section.collapsed);
        return (
          <div key={section.key} className="db-done-section">
            <button
              type="button"
              className="db-done-section-head"
              aria-expanded={!collapsed}
              aria-label={`${section.title} (${section.cards.length})`}
              onClick={() => {
                if (!expandAll) setToggled((current) => ({ ...current, [section.key]: !collapsed }));
              }}
            >
              <span className="material-icons" aria-hidden="true">expand_more</span>
              <span className="db-done-section-title">{section.title}</span>
              <span className="db-done-section-count">{section.cards.length}</span>
            </button>
            {!collapsed && section.cards.map((card) => <DbDoneRow key={card.id} card={card} onOpen={onOpen} merge={merges?.get(card.id)} now={now} />)}
          </div>
        );
      })}
      {layout.archived > 0 && <p className="db-done-archived">{layout.archived} im Archiv</p>}
    </>
  );
}
