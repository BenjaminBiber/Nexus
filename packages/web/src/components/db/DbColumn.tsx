import { useState } from 'react';
import type { DbCardDto, DbColumnDto, DbStatus } from '../../api/dbTypes.js';
import type { DispatchMergeDto } from '../../api/dispatchTypes.js';
import { DbCard } from './DbCard.js';

export const WAITING_HINT = '„Wartet“ ergibt sich aus offenen Voraussetzungen und nimmt keine Karten an. Voraussetzungen im Ticket unter „Setzt voraus“ ändern.';

// Spalte des DB-Boards mit Titel, Zähler und Karten; „Wartet“ ist gesperrt und meldet einen Ablageversuch.
export function DbColumn(props: {
  column: DbColumnDto;
  cards: DbCardDto[];
  onMove: (id: string, status: DbStatus) => void;
  onOpen: (id: string) => void;
  onLockedDrop: () => void;
  agentPrefix?: string;
  compact?: boolean;
  /** Laufende Merges des Dispatchers: Ticket-ID → Phase */
  merges?: ReadonlyMap<string, DispatchMergeDto['phase']>;
  /** Tickets, an denen gerade ein Agent-Lauf arbeitet */
  activeRuns?: ReadonlySet<string>;
}) {
  const { column } = props;
  const locked = column.id === 'waiting';
  const [over, setOver] = useState(false);
  return (
    <section
      className={`column db-column${locked ? ' locked' : ''}${over ? (locked ? ' drop-blocked' : ' drop') : ''}`}
      aria-label={column.title}
      data-status={column.id}
      title={locked ? WAITING_HINT : undefined}
      onDragOver={(event) => {
        event.preventDefault();
        if (locked && event.dataTransfer) event.dataTransfer.dropEffect = 'none';
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(event) => {
        event.preventDefault();
        setOver(false);
        if (locked) return props.onLockedDrop();
        const id = event.dataTransfer.getData('text/plain');
        if (id) props.onMove(id, column.id as DbStatus);
      }}
    >
      <div className="column-head">
        <span className="column-dot" aria-hidden="true" />
        <h2>{column.title}</h2>
        <span className="count">{props.cards.length}</span>
      </div>
      {props.cards.length === 0 && <div className="column-empty">Keine Tickets</div>}
      {props.cards.map((card) => (
        <DbCard key={card.id} card={card} onOpen={props.onOpen} onMove={props.onMove} agentPrefix={props.agentPrefix} compact={props.compact} merge={props.merges?.get(card.id)} active={props.activeRuns?.has(card.id)} />
      ))}
    </section>
  );
}
