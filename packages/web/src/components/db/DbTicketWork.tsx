import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { useApi } from '../../api/ApiContext.js';
import type { DbCommentDto, DbTicketDto } from '../../api/dbTypes.js';

// Herkunft und Umsetzung eines DB-Tickets: Spec, Zielbranch, Branch, Worktree, Claude-Account des letzten Laufs, Bereiche,
// Schnittstellen und Sessions (nur lesend; Seitenleiste des Ticket-Panels).
export function DbTicketWork({ ticket }: { ticket: DbTicketDto & { comments?: DbCommentDto[] } }) {
  const { pid } = useParams();
  const rows: Array<{ label: string; value: string; mono?: boolean; title?: string }> = [];
  const account = lastAccount(ticket.comments ?? []);
  const accountName = useAccountName(account?.id ?? null);
  if (ticket.spec) rows.push({ label: 'Spec', value: ticket.specTask ? `${ticket.spec} / ${ticket.specTask}` : ticket.spec });
  if (ticket.targetBranch) rows.push({ label: 'Zielbranch', value: ticket.targetBranch, mono: true });
  if (ticket.branch) rows.push({ label: 'Branch', value: ticket.branch, mono: true });
  if (ticket.worktree) rows.push({ label: 'Worktree', value: ticket.worktree, mono: true });
  if (account) rows.push({ label: 'Account', value: accountName ?? account.id, title: [account.id, account.why].filter(Boolean).join(' – ') });
  if (ticket.touches.length) rows.push({ label: 'Bereiche', value: ticket.touches.join(', '), mono: true });
  if (ticket.consumes.length) rows.push({ label: 'Nutzt', value: ticket.consumes.join('; ') });
  if (ticket.produces.length) rows.push({ label: 'Liefert', value: ticket.produces.join('; ') });
  if (!rows.length && !ticket.sessions.length) return null;
  return (
    <section className="db-work db-side-section" aria-label="Umsetzung">
      <h3 className="db-side-title">Umsetzung</h3>
      <dl className="db-work-list">
        {rows.map((row) => (
          <div key={row.label}>
            <dt>{row.label}</dt>
            <dd className={row.mono ? 'mono' : undefined} title={row.title}>{row.value}</dd>
          </div>
        ))}
        {ticket.sessions.length > 0 && (
          <div>
            <dt>Sessions</dt>
            <dd>
              {ticket.sessions.map((s) => (
                <span key={s} className="mono db-session" title={s}>
                  {s.slice(0, 8)}
                </span>
              ))}
              {pid && <Link to={`/p/${pid}/sessions`}>Sessions ansehen</Link>}
            </dd>
          </div>
        )}
      </dl>
    </section>
  );
}

// Account des letzten Laufs aus dem jüngsten Start-Kommentar des Dispatchers („Account: max (Profil schwer → max).“)
function lastAccount(comments: DbCommentDto[]): { id: string; why: string | null } | null {
  for (const c of [...comments].reverse()) {
    if (!c.body.startsWith('[dispatch] Start')) continue;
    const match = /^Account: ([a-z0-9-]+)(?: \((.*)\))?\.?$/m.exec(c.body);
    return match ? { id: match[1], why: match[2] ?? null } : null;
  }
  return null;
}

// Anzeigename eines Accounts; bis zur Antwort, ohne Server-Unterstützung oder für einen entfernten Account null (dann die ID)
function useAccountName(id: string | null): string | null {
  const api = useApi();
  const [name, setName] = useState<string | null>(null);
  useEffect(() => {
    setName(null);
    if (!id) return;
    let current = true;
    api.getAccountNames().then(
      (list) => { if (current) setName(list.accounts.find((a) => a.id === id)?.name ?? null); },
      () => undefined,
    );
    return () => { current = false; };
  }, [api, id]);
  return name;
}
