import type { DbCardDto } from '../../api/dbTypes.js';
import { MERGE_PHASE_LABELS, type DispatchMergeDto } from '../../api/dispatchTypes.js';
import { formatDoneDate } from '../../dbboard/doneSections.js';
import { Card } from '../ui/Card.js';

// Kompakte Zeile eines erledigten Tickets: oben ID und Erledigt-Datum, darunter der Titel (höchstens zwei Zeilen).
// Die Spalten des Boards sind ab 180 px breit; in einer einzigen Zeile bliebe vom Titel dort nichts übrig.
// Ziehbar wie eine Karte, der Klick öffnet das Ticket. Mergt der Dispatcher das Ticket gerade (merge: Phase), steht vor dem Datum die Pille „Merge“.
export function DbDoneRow({ card, onOpen, merge, now }: { card: DbCardDto; onOpen: (id: string) => void; merge?: DispatchMergeDto['phase']; now: Date }) {
  // Ohne Erledigt-Datum (älterer Server) gilt die letzte Änderung
  const at = card.doneAt ?? card.updatedAt;
  const date = new Date(at);
  const readable = !Number.isNaN(date.getTime());
  return (
    <Card
      as="article"
      className="card db-card db-done-row"
      draggable
      data-id={card.id}
      onDragStart={(event) => {
        event.dataTransfer.setData('text/plain', card.id);
        event.dataTransfer.effectAllowed = 'move';
      }}
    >
      <button type="button" className="db-card-open db-done-open" onClick={() => onOpen(card.id)}>
        <span className="db-done-meta">
          <span className="card-id">{card.id}</span>
          {merge && <span className="badge running" title={MERGE_PHASE_LABELS[merge]}>Merge</span>}
          {readable && (
            <time className="db-done-date" dateTime={at} title={date.toLocaleString('de-DE')}>
              {formatDoneDate(at, now)}
            </time>
          )}
        </span>
        <span className="db-done-title" title={card.title}>{card.title}</span>
      </button>
    </Card>
  );
}
