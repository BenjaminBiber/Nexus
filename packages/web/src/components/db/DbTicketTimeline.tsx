import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { useApi } from '../../api/ApiContext.js';
import type { DbTicketTimelineDto, DbTimelineEventDto, DbTimelineKind } from '../../api/dbTypes.js';
import { dayLabel, timeLabel, timelineView } from '../../dbboard/timelineView.js';
import { MarkdownView } from '../../markdown/MarkdownView.js';
import { Collapsible } from '../Collapsible.js';
import { DbCommentButton } from './DbCommentButton.js';

export { FIELD_LABELS } from '../../dbboard/timelineView.js';

// Arten der Zeitleiste in Anzeige-Reihenfolge der Filter
export const TIMELINE_KIND_LABELS: Record<DbTimelineKind, string> = {
  status: 'Status', claim: 'Zuständig', run: 'Läufe', question: 'Fragen', git: 'Git', comment: 'Kommentare', content: 'Inhalte', other: 'Sonstiges',
};
const KINDS = Object.keys(TIMELINE_KIND_LABELS) as DbTimelineKind[];

const FILTER_KEY = 'harness.timeline.hidden';
const ORDER_KEY = 'harness.timeline.oldestFirst';

// Gemerkte Ansicht lesen und schreiben; ohne Speicher (privates Fenster, Vorschau) gilt der Standard
function readStored<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}
function writeStored(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* nur Bequemlichkeit */
  }
}

// Ereignisse nach Tag gruppiert, in der gewählten Reihenfolge
function byDay(events: DbTimelineEventDto[]): Array<{ day: string; events: DbTimelineEventDto[] }> {
  const groups: Array<{ day: string; events: DbTimelineEventDto[] }> = [];
  for (const e of events) {
    const day = dayLabel(e.at);
    const last = groups.at(-1);
    if (last && last.day === day) last.events.push(e);
    else groups.push({ day, events: [e] });
  }
  return groups;
}

// Knopf in der Leiste der Zeitleiste, der den Kommentardialog der Reiter öffnet
export interface TimelineComposer {
  onOpen: () => void;
  busy: boolean;
}

// Zeitleiste eines Tickets im Reiter „Verlauf“: oben eine Leiste mit Kommentarknopf und Filtern, darunter senkrechte Linie mit farbigen Punkten,
// Tage als Zwischenüberschrift, Uhrzeit rechts; Läufe, Kommentare, Merge, Entscheidungen und Fehler hervorgehoben, lange
// Texte eingeklappt. Filterbar nach Art, standardmäßig neueste zuerst. Lädt bei jeder neuen Version des Tickets neu.
export function DbTicketTimeline({ ticketId, rev, composer }: { ticketId: string; rev: number; composer?: TimelineComposer }) {
  const api = useApi();
  const { pid } = useParams();
  const [data, setData] = useState<DbTicketTimelineDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [hidden, setHidden] = useState<DbTimelineKind[]>(() => readStored<DbTimelineKind[]>(FILTER_KEY, []));
  const [oldestFirst, setOldestFirst] = useState<boolean>(() => readStored(ORDER_KEY, false));

  useEffect(() => {
    let stale = false;
    api.getDbTicketTimeline(ticketId).then(
      (result) => {
        if (stale) return;
        setData(result);
        setError(null);
      },
      (e: unknown) => { if (!stale) setError(e instanceof Error ? e.message : String(e)); },
    );
    return () => { stale = true; };
  }, [api, ticketId, rev]);

  const toggleKind = (kind: DbTimelineKind) => {
    const next = hidden.includes(kind) ? hidden.filter((k) => k !== kind) : [...hidden, kind];
    setHidden(next);
    writeStored(FILTER_KEY, next);
  };
  const toggleOrder = () => {
    setOldestFirst(!oldestFirst);
    writeStored(ORDER_KEY, !oldestFirst);
  };

  if (error && !data) return <p role="alert" className="field-error">Verlauf nicht geladen: {error}</p>;
  if (!data) return <p className="muted">Verlauf wird geladen …</p>;

  const counts = new Map<DbTimelineKind, number>();
  for (const e of data.events) counts.set(e.kind, (counts.get(e.kind) ?? 0) + 1);
  const shown = data.events.filter((e) => !hidden.includes(e.kind));
  const ordered = oldestFirst ? shown : [...shown].reverse();

  return (
    <div className="db-timeline">
      <div className="db-timeline-bar">
        {composer && <DbCommentButton onOpen={composer.onOpen} busy={composer.busy} />}
        <div className="chip-group" role="group" aria-label="Arten filtern">
          {KINDS.filter((k) => counts.has(k)).map((k) => (
            <button key={k} type="button" className={`fchip${hidden.includes(k) ? '' : ' on'}`} aria-pressed={!hidden.includes(k)} onClick={() => toggleKind(k)}>
              {TIMELINE_KIND_LABELS[k]} ({counts.get(k)})
            </button>
          ))}
        </div>
        <button type="button" className="btn" onClick={toggleOrder}>
          Reihenfolge: {oldestFirst ? 'älteste zuerst' : 'neueste zuerst'}
        </button>
      </div>
      {(data.sources.runs !== 'ok' || data.sources.git !== 'ok') && (
        <p className="muted db-timeline-note">
          {data.sources.runs !== 'ok' && 'Lauf-Protokolle gibt es nur auf dem Rechner des Dispatchers. '}
          {data.sources.git !== 'ok' && `Commits nicht lesbar${data.sources.gitMessage ? `: ${data.sources.gitMessage}` : '.'}`}
        </p>
      )}
      {ordered.length === 0 && <p className="muted">{data.events.length ? 'Alle Arten ausgeblendet.' : 'Noch kein Verlauf.'}</p>}
      <div className="tl">
        {byDay(ordered).map((group, g) => (
          <section key={`${group.day}-${g}`} aria-label={group.day} className="tl-day">
            <h4 className="tl-day-title">{group.day}</h4>
            <ol className="tl-list">
              {group.events.map((e, index) => {
                const v = timelineView(e);
                return (
                  <li key={`${e.at}-${e.type}-${index}`} className={`tl-item kind-${e.kind}`}>
                    <span className={`tl-dot tone-${v.tone}`} aria-hidden="true" />
                    <div className={`tl-body${v.highlight ? ' highlight' : ''}`}>
                      <div className="tl-head">
                        <span className="tl-title">{v.title}</span>
                        <time className="tl-time" dateTime={e.at} title={new Date(e.at).toLocaleString('de-DE')}>{timeLabel(e.at)}</time>
                      </div>
                      {v.markdown !== undefined && (
                        <Collapsible text={v.markdown}>
                          <MarkdownView source={v.markdown} />
                        </Collapsible>
                      )}
                      <div className="tl-meta">
                        <span className="sr-only">{TIMELINE_KIND_LABELS[e.kind]} · </span>
                        {v.meta.join(' · ')}
                      </div>
                      {v.error && <Collapsible text={v.error} className="tl-error">{v.error}</Collapsible>}
                      {v.warning && <Collapsible text={v.warning} className="tl-error warn">{v.warning}</Collapsible>}
                      {v.detail && <Collapsible text={v.detail} className="tl-detail">{v.detail}</Collapsible>}
                      {v.questionId && pid && (
                        <Link className="tl-action" to={`/p/${encodeURIComponent(pid)}/questions/${encodeURIComponent(v.questionId)}`}>
                          Frage {v.questionId} ansehen
                        </Link>
                      )}
                    </div>
                  </li>
                );
              })}
            </ol>
          </section>
        ))}
      </div>
    </div>
  );
}
