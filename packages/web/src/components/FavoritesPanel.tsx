import { useEffect } from 'react';
import type { FavoriteDto } from '../api/types.js';
import { EmptyState } from './EmptyState.js';
import { FavoriteItem } from './FavoriteItem.js';

interface Props {
  pid: string;
  favorites: FavoriteDto[];
  now: number;
  onNote: (sessionId: string, note: string) => Promise<void>;
  onRemove: (sessionId: string) => Promise<void>;
  onClose: () => void;
}

// Favoriten als Seitenfenster von rechts, mit Leerzustand, der das Markieren erklärt.
export function FavoritesPanel({ pid, favorites, now, onNote, onRemove, onClose }: Props) {
  // Mit Escape schließen
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => event.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  return (
    <>
      <div className="sheet-backdrop" onClick={onClose} aria-hidden="true" />
      <aside id="favpanel" className="sheet" role="dialog" aria-label="Favoriten">
        <div className="sheet-head">
          <div>
            <h2>Favoriten</h2>
            <p className="muted">Markierte Sessions bleiben unbegrenzt erhalten. Mit dem Befehl setzt du sie später fort.</p>
          </div>
          <button type="button" className="star" aria-label="Favoriten schließen" onClick={onClose}>
            <svg width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
              <path d="M5 5l10 10M15 5L5 15" />
            </svg>
          </button>
        </div>
        <div className="sheet-body">
          {favorites.length === 0 ? (
            <EmptyState title="Noch keine Favoriten">
              Markiere eine Session mit dem Stern. Dann bleibt sie erhalten, und du bekommst den Befehl zum Fortsetzen.
            </EmptyState>
          ) : (
            favorites.map((f) => <FavoriteItem key={f.sessionId} pid={pid} favorite={f} now={now} onNote={onNote} onRemove={onRemove} />)
          )}
        </div>
      </aside>
    </>
  );
}
