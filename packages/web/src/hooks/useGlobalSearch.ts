import { useEffect, useRef, useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import type { DbSearchResultDto } from '../api/dbTypes.js';

export type GlobalSearchState =
  | { status: 'idle' }
  | { status: 'loading'; previous: DbSearchResultDto | null }
  | { status: 'ready'; result: DbSearchResultDto }
  | { status: 'error'; message: string };

const MIN_LENGTH = 2;

// Suche über alle Boards mit Verzögerung beim Tippen; eine ältere Antwort überschreibt nie eine neuere.
export function useGlobalSearch(query: string, delayMs = 250): GlobalSearchState {
  const api = useApi();
  const [state, setState] = useState<GlobalSearchState>({ status: 'idle' });
  const latest = useRef(0);
  const last = useRef<DbSearchResultDto | null>(null);

  useEffect(() => {
    const q = query.trim();
    const ticket = ++latest.current;
    if (q.length < MIN_LENGTH) {
      setState({ status: 'idle' });
      return;
    }
    setState({ status: 'loading', previous: last.current });
    const timer = setTimeout(() => {
      api.searchDb(q).then(
        (result) => {
          if (ticket !== latest.current) return;
          last.current = result;
          setState({ status: 'ready', result });
        },
        (e: unknown) => {
          if (ticket === latest.current) setState({ status: 'error', message: e instanceof Error ? e.message : String(e) });
        },
      );
    }, delayMs);
    return () => clearTimeout(timer);
  }, [api, query, delayMs]);

  return state;
}
