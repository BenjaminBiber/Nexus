import type { ReactNode } from 'react';
import { Link } from 'react-router-dom';
import type { SessionDetail, SessionTodo, TodoStatus } from '../../api/sessionDetailTypes.js';
import { formatAgo, formatBytes, formatCost, formatDateTime, formatDuration, formatNumber, formatSeconds, formatTokens } from '../../sessions/format.js';
import { CopyIdButton } from '../CopyIdButton.js';
import { Progress } from '../Progress.js';
import { SessionStateBadge } from '../SessionStateBadge.js';
import { SubagentTree } from '../SubagentTree.js';
import { SessionClientBadge } from './SessionClientBadge.js';

// Zeile der Definitionsliste; ohne Wert erscheint ein Gedankenstrich
function Row({ term, children }: { term: string; children?: ReactNode }) {
  const empty = children === null || children === undefined || children === '';
  return (
    <div className="ri-row">
      <dt>{term}</dt>
      <dd>{empty ? <span className="muted">–</span> : children}</dd>
    </div>
  );
}

function Section({ title, meta, children }: { title: string; meta?: ReactNode; children: ReactNode }) {
  return (
    <section className="ri-section" aria-label={title}>
      <h3>
        {title}
        {meta && <span className="ri-meta">{meta}</span>}
      </h3>
      {children}
    </section>
  );
}

function Stat({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <div className="ri-stat" title={hint}>
      <span className="ri-stat-label">{label}</span>
      <span className="ri-stat-value mono">{value}</span>
    </div>
  );
}

const TODO: Record<TodoStatus, { icon: string; label: string }> = {
  completed: { icon: 'check_circle', label: 'erledigt' },
  in_progress: { icon: 'radio_button_checked', label: 'in Arbeit' },
  pending: { icon: 'radio_button_unchecked', label: 'offen' },
};

function Todos({ todos }: { todos: SessionTodo[] }) {
  return (
    <ul className="ri-todos">
      {todos.map((todo, index) => (
        <li key={index} className={`ri-todo ${todo.status}`}>
          <span className="material-icons" aria-hidden="true">
            {TODO[todo.status].icon}
          </span>
          <span className="ri-todo-text">{todo.content}</span>
          <span className="sr-only">({TODO[todo.status].label})</span>
        </li>
      ))}
    </ul>
  );
}

// Kontextbalken: ab 70 % gelb, ab 90 % rot
function Context({ used, limit }: { used: number; limit: number | null }) {
  if (limit === null || limit <= 0) return <p className="mono">{formatNumber(used)} Tokens im Kontext</p>;
  const percent = Math.round((used / limit) * 100);
  const tone = percent >= 90 ? 'crit' : percent >= 70 ? 'warn' : 'ok';
  return (
    <Progress
      done={used}
      total={limit}
      tone={tone}
      label={`Kontextauslastung ${percent} %`}
      text={
        <>
          <span className="mono">{formatNumber(used)}</span> von <span className="mono">{formatNumber(limit)}</span> Tokens · {percent} %
        </>
      }
    />
  );
}

interface Props {
  pid: string;
  detail: SessionDetail;
  now: number;
  onOpenLog: (name: string) => void;
}

// Tab Lauf: Basisdaten, Verbrauch, Kontext, Todos, Subagenten und bei Nexus-Läufen die Laufdaten des Dispatchers.
export function SessionRunInfo({ pid, detail, now, onOpenLog }: Props) {
  const { session, tokens, run } = detail;
  const done = detail.todos.filter((t) => t.status === 'completed').length;
  const runName = run?.name ?? session.runName;
  const tickets = session.ticketInfo.length ? session.ticketInfo : session.tickets.map((id) => ({ id, title: '', status: '' }));
  return (
    <div className="ri">
      {runName && (
        <Section title="Nexus-Lauf">
          <dl className="ri-list">
            <Row term="Protokoll">
              <span className="mono">{runName}</span>{' '}
              <button type="button" className="btn small" onClick={() => onOpenLog(runName)}>
                Protokoll öffnen
              </button>
            </Row>
            <Row term="Art">{run?.kind}</Row>
            <Row term="Profil">{run?.profile}</Row>
            <Row term="Konto">{run?.account}</Row>
            <Row term="Ergebnis">{run?.outcome ?? (run?.startedAt && !run.endedAt ? <span className="muted">läuft noch</span> : null)}</Row>
            {run?.abortReason && <Row term="Abbruchgrund"><span className="ri-abort">{run.abortReason}</span></Row>}
            <Row term="Start">{run?.startedAt ? formatDateTime(run.startedAt) : null}</Row>
            {run?.endedAt && <Row term="Ende">{formatDateTime(run.endedAt)}</Row>}
          </dl>
        </Section>
      )}

      <Section title="Session">
        <dl className="ri-list">
          <Row term="Quelle"><SessionClientBadge client={detail.client} /></Row>
          <Row term="Zustand"><SessionStateBadge state={session.displayState} /></Row>
          <Row term="Modell">{session.model && <span className="mono">{session.model}</span>}</Row>
          <Row term="Claude Code">{detail.claudeVersion && <span className="mono">{detail.claudeVersion}</span>}</Row>
          <Row term="Gestartet">{formatDateTime(session.startedAt)}</Row>
          <Row term="Letzte Aktivität">
            <span title={formatDateTime(session.lastEventAt)}>{formatAgo(session.lastEventAt, now)}</span>
          </Row>
          {session.endedAt && <Row term="Beendet">{formatDateTime(session.endedAt)}</Row>}
          <Row term="Laufzeit">{formatDuration(session.runtimeMs)}</Row>
          <Row term="Branch">{detail.branch && <span className="mono">{detail.branch}</span>}</Row>
          <Row term="Arbeitsverzeichnis">
            <span className="ri-copy">
              <span className="mono ri-path">{session.cwd}</span>
              <CopyIdButton id={session.cwd} label="Arbeitsverzeichnis" caption="Kopieren" />
            </span>
          </Row>
          <Row term="Session-ID">
            <span className="ri-copy">
              <span className="mono ri-path">{session.id}</span>
              <CopyIdButton id={session.id} label="Session-ID" caption="Kopieren" />
            </span>
          </Row>
          <Row term="Tickets">
            {tickets.length > 0 && (
              <span className="ri-tickets">
                {tickets.map((t) => (
                  <Link key={t.id} to={`/p/${pid}/board/t/${encodeURIComponent(t.id)}`} title={t.title || undefined} className="mono">
                    {t.id}
                  </Link>
                ))}
              </span>
            )}
          </Row>
          <Row term="Transcript">
            {detail.transcript.available && detail.transcript.path ? (
              <span className="mono ri-path" title={detail.transcript.path}>
                {detail.transcript.path}
                {detail.transcript.sizeBytes !== null && <span className="muted"> · {formatBytes(detail.transcript.sizeBytes)}</span>}
              </span>
            ) : (
              <span className="muted">nicht gefunden</span>
            )}
          </Row>
        </dl>
      </Section>

      <Section title="Verbrauch">
        <div className="ri-stats">
          <Stat label="Input" value={formatTokens(tokens.input)} hint={`${formatNumber(tokens.input)} Tokens`} />
          <Stat label="Output" value={formatTokens(tokens.output)} hint={`${formatNumber(tokens.output)} Tokens`} />
          <Stat label="Cache lesen" value={formatTokens(tokens.cacheRead)} hint={`${formatNumber(tokens.cacheRead)} Tokens`} />
          <Stat label="Cache schreiben" value={formatTokens(tokens.cacheWrite)} hint={`${formatNumber(tokens.cacheWrite)} Tokens`} />
          <Stat label="Kosten" value={detail.costUsd !== null ? formatCost(detail.costUsd) : '–'} />
          <Stat label="API-Dauer" value={detail.durationMs !== null ? formatSeconds(detail.durationMs) : '–'} hint="Gesamtdauer laut Claude Code" />
        </div>
      </Section>

      <Section title="Kontext">
        {detail.context ? <Context used={detail.context.usedTokens} limit={detail.context.limitTokens} /> : <p className="muted">Noch keine Antwort mit Kontextangabe.</p>}
      </Section>

      <Section title="Todos" meta={detail.todos.length ? `${done} von ${detail.todos.length} erledigt` : undefined}>
        {detail.todos.length ? <Todos todos={detail.todos} /> : <p className="muted">Keine Todos.</p>}
      </Section>

      <Section title="Subagenten" meta={session.subagents.length ? String(session.subagents.length) : undefined}>
        {session.subagents.length ? (
          <SubagentTree agents={session.subagents} label={session.id.slice(0, 8)} now={now} showEnded />
        ) : (
          <p className="muted">Keine Subagenten.</p>
        )}
      </Section>
    </div>
  );
}
