import { useApi } from '../api/ApiContext.js';
import { useLiveHandler } from '../live/useLive.js';
import { useAsync } from './useAsync.js';

// Lädt die Sessions eines Projekts und aktualisiert sie bei Live-Meldungen.
export function useSessions(pid: string) {
  const api = useApi();
  const loaded = useAsync(() => api.getSessions(pid), [api, pid]);
  useLiveHandler({ onSessions: loaded.reload, onReset: loaded.reload });
  return loaded;
}
