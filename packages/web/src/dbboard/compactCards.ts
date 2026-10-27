import { useEffect } from 'react';
import { useApi } from '../api/ApiContext.js';
import { useAsync } from '../hooks/useAsync.js';

/** Meldung der Einstellungen an das Board: der Schalter hat sich geändert, sofort neu laden */
export const COMPACT_CARDS_EVENT = 'harness:compact-cards';

// Liest die Einstellung „Kompakte Ticket-Kacheln“ und lädt sie nach, sobald der Schalter umgelegt wurde. Ein Fehlschlag
// darf das Board nicht blockieren, deshalb bleibt es dann bei der normalen Ansicht.
export function useCompactCards(): boolean {
  const api = useApi();
  const loaded = useAsync(() => api.getUserSettings().catch(() => null), [api]);
  const reload = loaded.reload;
  useEffect(() => {
    window.addEventListener(COMPACT_CARDS_EVENT, reload);
    return () => window.removeEventListener(COMPACT_CARDS_EVENT, reload);
  }, [reload]);
  return loaded.state.status === 'ready' ? (loaded.state.data?.compactCards ?? false) : false;
}
