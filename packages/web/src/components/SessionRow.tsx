import type { MouseEvent } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import type { SessionDto } from '../api/types.js';
import { formatAgo, formatDuration } from '../sessions/format.js';
import { CopyIdButton } from './CopyIdButton.js';
import { FavoriteStar } from './FavoriteStar.js';
import { SessionStateBadge } from './SessionStateBadge.js';
import { SubagentTree } from './SubagentTree.js';

// Eine Session als Zeile: Zustand, Laufzeit, letzte Aktivität, Arbeitsverzeichnis, Stern und Subagenten.
// Mit `href` öffnet der Titel (bzw. ein Klick auf eine freie Stelle der Zeile) die Details; Stern, Kopieren und Links bleiben, wie sie sind.
export function SessionRow(props: { session: SessionDto; now: number; href?: string; favorite?: boolean; showEndedAgents?: boolean; onToggleFavorite?: (session: SessionDto) => void }) {
  const { session, now } = props;
  const short = session.id.slice(0, 8);
  const dim = session.displayState === 'stale' || session.displayState === 'ended';
  const classes = ['session-row', session.displayState === 'waiting' ? 'waiting' : '', dim ? `dim ${session.displayState}` : '', props.href ? 'openable' : ''].join(' ').trim();
  const navigate = useNavigate();
  const { href } = props;
  // Klick auf eine freie Stelle der Zeile öffnet die Details (Tastatur: über den Titel-Link)
  const onRowClick = (event: MouseEvent<HTMLElement>) => {
    if (!href || event.defaultPrevented || (event.target as Element).closest('a, button, input, select, textarea, label')) return;
    if (window.getSelection()?.toString()) return;
    navigate(href, { state: { fromList: true } });
  };
  return (
    <article className={classes} onClick={onRowClick}>
      <span className={`state-dot ${session.displayState}`} aria-hidden="true" />
      <SessionStateBadge state={session.displayState} />
      <span className="mono">{formatDuration(session.runtimeMs)}</span>
      <span className="activity">
        {href ? (
          <Link to={href} state={{ fromList: true }} aria-label={`${session.title || 'Ohne Titel'} · Session ${short} öffnen`} className={`session-title session-open${session.title ? '' : ' untitled'}`}>
            {session.title || 'Ohne Titel'}
          </Link>
        ) : (
          session.title && <span className="session-title">{session.title}</span>
        )}
        <span className="session-meta muted">
          <span className="mono" title={session.id}>{short}</span> <CopyIdButton id={session.id} label="Session-ID" /> · Letzte Aktivität {formatAgo(session.lastEventAt, now)}
        </span>
      </span>
      <span className="mono cwd">{session.cwd}</span>
      {props.onToggleFavorite ? <FavoriteStar active={props.favorite === true} onToggle={() => props.onToggleFavorite!(session)} /> : <span />}
      <SubagentTree agents={session.subagents} label={short} now={now} showEnded={props.showEndedAgents} />
    </article>
  );
}
