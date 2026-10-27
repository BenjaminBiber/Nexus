import type { SessionDto } from '../api/types.js';
import { formatAgo, formatDuration } from '../sessions/format.js';
import { CopyIdButton } from './CopyIdButton.js';
import { FavoriteStar } from './FavoriteStar.js';
import { SessionStateBadge } from './SessionStateBadge.js';
import { SubagentTree } from './SubagentTree.js';

// Eine Session als Zeile: Zustand, Laufzeit, letzte Aktivität, Arbeitsverzeichnis, Stern und Subagenten.
export function SessionRow(props: { session: SessionDto; now: number; favorite?: boolean; showEndedAgents?: boolean; onToggleFavorite?: (session: SessionDto) => void }) {
  const { session, now } = props;
  const short = session.id.slice(0, 8);
  const dim = session.displayState === 'stale' || session.displayState === 'ended';
  const classes = ['session-row', session.displayState === 'waiting' ? 'waiting' : '', dim ? `dim ${session.displayState}` : ''].join(' ').trim();
  return (
    <article className={classes}>
      <span className={`state-dot ${session.displayState}`} aria-hidden="true" />
      <SessionStateBadge state={session.displayState} />
      <span className="mono">{formatDuration(session.runtimeMs)}</span>
      <span className="activity">
        {session.title && <span className="session-title">{session.title}</span>}
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
