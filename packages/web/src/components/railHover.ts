import { useEffect } from 'react';
import { useApiOrNull } from '../api/ApiContext.js';
import type { RailHover } from '../api/types.js';
import { useAsync } from '../hooks/useAsync.js';

/** Meldung der Einstellungen an die Seitenleiste: die Auswahl hat sich geändert, sofort neu laden */
export const RAIL_HOVER_EVENT = 'harness:rail-hover';

// Liest, wie die eingeklappte Seitenleiste unter dem Zeiger reagiert, und lädt nach, sobald die Auswahl wechselt. Bis
// die Einstellungen da sind (ohne Server oder wenn sie fehlschlagen) bleibt es bei der herausfließenden Pille.
export function useRailHover(): RailHover {
  const api = useApiOrNull();
  const loaded = useAsync(async () => (api ? api.getUserSettings().catch(() => null) : null), [api]);
  const reload = loaded.reload;
  useEffect(() => {
    window.addEventListener(RAIL_HOVER_EVENT, reload);
    return () => window.removeEventListener(RAIL_HOVER_EVENT, reload);
  }, [reload]);
  return loaded.state.status === 'ready' ? (loaded.state.data?.railHover ?? 'liquid') : 'liquid';
}
