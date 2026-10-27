import { useState } from 'react';
import type { DocTicketRefDto } from '../api/types.js';
import { DocTicketChip } from './DocHeadingTickets.js';
import { ProgressRing } from './ProgressRing.js';

export type DocTicketGroup = 'open' | 'blocked' | 'done';

const TABS: Array<{ key: DocTicketGroup; label: string; empty: string }> = [
  { key: 'open', label: 'Offen', empty: 'Keine offenen Tickets.' },
  { key: 'blocked', label: 'Blockiert', empty: 'Keine blockierten Tickets.' },
  { key: 'done', label: 'Erledigt', empty: 'Keine erledigten Tickets.' },
];

// Gruppe eines Tickets: „entfällt“ zählt als erledigt, alles vor dem Abschluss (auch „wartet“) als offen
export function docTicketGroup(ticket: DocTicketRefDto): DocTicketGroup {
  if (ticket.status === 'done' || ticket.status === 'not_needed') return 'done';
  if (ticket.status === 'blocked') return 'blocked';
  return 'open';
}

// Tickets aus diesem Dokument: Fortschritt als Ring, darunter Offen/Blockiert/Erledigt als Tabs.
// Der Titel springt zur Überschrift, aus der das Ticket entstand, sofern es sie noch gibt.
export function DocTicketPanel({ pid, tickets, canJump, onJump }: {
  pid: string;
  tickets: DocTicketRefDto[];
  canJump: (ticket: DocTicketRefDto) => boolean;
  onJump: (ticket: DocTicketRefDto) => void;
}) {
  const [tab, setTab] = useState<DocTicketGroup>('open');
  const groups = { open: [] as DocTicketRefDto[], blocked: [] as DocTicketRefDto[], done: [] as DocTicketRefDto[] };
  tickets.forEach((t) => groups[docTicketGroup(t)].push(t));
  const shown = groups[tab];

  return (
    <div className="doc-ticket-panel">
      <ProgressRing done={groups.done.length} total={tickets.length} label="Fortschritt der Tickets" caption="Tickets erledigt" />
      <div className="doc-comment-tabs doc-ticket-tabs">
        {TABS.map((t) => (
          <button key={t.key} type="button" aria-pressed={tab === t.key} onClick={() => setTab(t.key)}>
            {t.label} ({groups[t.key].length})
          </button>
        ))}
      </div>
      {shown.length === 0 && <p className="muted">{TABS.find((t) => t.key === tab)?.empty}</p>}
      <ul className="doc-ticket-list" aria-label="Tickets zu diesem Dokument">
        {shown.map((t) => (
          <li key={t.id}>
            <DocTicketChip pid={pid} ticket={t} />
            {canJump(t) ? (
              <button type="button" className="doc-ticket-title" title="Zur Stelle im Dokument" onClick={() => onJump(t)}>{t.title}</button>
            ) : (
              <span className="doc-ticket-title muted">{t.title}</span>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}
