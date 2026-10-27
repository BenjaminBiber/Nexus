import { useCallback, useEffect, useRef, useState } from 'react';

export type AsyncState<T> =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'ready'; data: T };

// Lädt Daten asynchron (neu bei Änderung von `deps`); `reload` lädt ohne Ladeanzeige nach, `set` ersetzt die Daten.
// `reload` und `retry` liefern die geladenen Daten (undefined bei Fehler oder wenn ein neuerer Ladevorgang gewinnt).
export function useAsync<T>(load: () => Promise<T>, deps: unknown[]) {
  const [state, setState] = useState<AsyncState<T>>({ status: 'loading' });
  const counter = useRef(0);
  const loader = useRef(load);
  loader.current = load;

  const run = useCallback((showLoading: boolean): Promise<T | undefined> => {
    const id = ++counter.current;
    if (showLoading) setState({ status: 'loading' });
    return loader.current().then(
      (data) => {
        if (id !== counter.current) return undefined;
        setState({ status: 'ready', data });
        return data;
      },
      (error: Error) => {
        if (id === counter.current) setState({ status: 'error', message: error.message });
        return undefined;
      },
    );
  }, []);

  useEffect(() => {
    void run(true);
  }, deps); // eslint-disable-line react-hooks/exhaustive-deps
  const reload = useCallback(() => run(false), [run]);
  const retry = useCallback(() => run(true), [run]);
  // Lädt im Hintergrund nach und behält bei einem Fehler die bisherigen Daten; liefert die Fehlermeldung oder null
  const refresh = useCallback((): Promise<string | null> => {
    const id = ++counter.current;
    return loader.current().then(
      (data) => {
        if (id === counter.current) setState({ status: 'ready', data });
        return null;
      },
      (error: Error) => (id === counter.current ? error.message : null),
    );
  }, []);
  // Setzt die Daten direkt und verwirft noch laufende Ladevorgänge, damit sie die Daten nicht überschreiben
  const set = useCallback((data: T) => {
    counter.current++;
    setState({ status: 'ready', data });
  }, []);
  return { state, reload, retry, refresh, set };
}
