import { useState, type FormEvent } from 'react';
import { Link } from 'react-router-dom';
import type { FavoriteDto } from '../api/types.js';
import { formatAgo } from '../sessions/format.js';
import { CopyCommand } from './CopyCommand.js';
import { SessionStateBadge } from './SessionStateBadge.js';

interface Props {
  pid: string;
  favorite: FavoriteDto;
  now: number;
  onNote: (sessionId: string, note: string) => Promise<void>;
  onRemove: (sessionId: string) => Promise<void>;
}

// Ein Favorit: Notiz (bearbeitbar), Tickets, Arbeitsverzeichnis, Zustand und der Befehl zum Fortsetzen.
export function FavoriteItem({ pid, favorite, now, onNote, onRemove }: Props) {
  const [note, setNote] = useState(favorite.note);
  const [saved, setSaved] = useState(false);
  const short = favorite.sessionId.slice(0, 8);

  // Speichert die Notiz und meldet es kurz
  const save = async (event: FormEvent) => {
    event.preventDefault();
    await onNote(favorite.sessionId, note);
    setSaved(true);
    setTimeout(() => setSaved(false), 1500);
  };

  return (
    <article className="fav-card" aria-label={`Favorit ${short}`}>
      <div className="fav-head">
        {favorite.ticketInfo.length > 0 ? (
          <span>
            {favorite.ticketInfo.map((t) => (
              <Link key={t.id} to={`/p/${pid}/board/t/${encodeURIComponent(t.id)}`} className="fav-ticket">
                <span className="mono">{t.id}</span> {t.title}{' '}
              </Link>
            ))}
          </span>
        ) : (
          <span className="mono">{short}</span>
        )}
        <SessionStateBadge state={favorite.displayState} />
      </div>
      {favorite.title && <div>{favorite.title}</div>}
      <form onSubmit={save} className="note-form">
        <label className="field grow">
          <span>Notiz</span>
          <input value={note} onChange={(e) => setNote(e.target.value)} placeholder="Wofür ist diese Session wichtig?" />
        </label>
        <button type="submit" className="btn">
          Notiz speichern
        </button>
        {saved && <span role="status">Gespeichert</span>}
      </form>
      <div className="fav-meta muted">
        <span>Letzte Aktivität {formatAgo(favorite.lastEventAt, now)}</span>
        <span className="mono cwd">{favorite.cwd}</span>
        {!favorite.live && <span>(nicht mehr in der Übersicht)</span>}
      </div>
      {!favorite.cwdExists && (
        <p role="alert" className="field-error">
          Arbeitsverzeichnis nicht mehr vorhanden. Der Befehl funktioniert erst, wenn es wieder existiert (z. B. neuer Worktree).
        </p>
      )}
      <CopyCommand command={favorite.command} />
      {favorite.resumedAs.length > 0 && (
        <p className="muted">Fortgesetzt als {favorite.resumedAs.map((id) => id.slice(0, 8)).join(', ')}</p>
      )}
      <div>
        <button type="button" className="btn danger" onClick={() => void onRemove(favorite.sessionId)}>
          Favorit entfernen
        </button>
      </div>
    </article>
  );
}
