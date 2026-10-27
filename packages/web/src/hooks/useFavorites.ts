import { useCallback } from 'react';
import { useApi } from '../api/ApiContext.js';
import { useLiveHandler } from '../live/useLive.js';
import { useAsync } from './useAsync.js';

// Lädt die Favoriten eines Projekts; Markieren, Notiz und Entfernen laden die Liste neu.
export function useFavorites(pid: string) {
  const api = useApi();
  const loaded = useAsync(() => api.getFavorites(pid), [api, pid]);
  useLiveHandler({ onSessions: loaded.reload, onReset: loaded.reload });

  // Markiert (oder ändert die Notiz) und lädt neu
  const mark = useCallback(
    async (sessionId: string, note?: string) => {
      await api.markFavorite(pid, sessionId, note);
      loaded.reload();
    },
    [api, pid, loaded.reload],
  );

  // Entfernt die Markierung und lädt neu
  const unmark = useCallback(
    async (sessionId: string) => {
      await api.unmarkFavorite(pid, sessionId);
      loaded.reload();
    },
    [api, pid, loaded.reload],
  );

  return { ...loaded, mark, unmark };
}
