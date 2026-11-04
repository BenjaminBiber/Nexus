import { useCallback, useEffect, useRef, useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import type { TranscriptEntry } from '../api/sessionDetailTypes.js';
import { useLiveHandler } from '../live/useLive.js';
import { useAsync } from './useAsync.js';

/** Einträge je Seite des Verlaufs; eine volle Seite heißt: es gibt vermutlich mehr */
export const TRANSCRIPT_PAGE_SIZE = 500;
/** Live-Nachladen höchstens so oft (ms) */
export const LIVE_THROTTLE_MS = 3000;

export interface TranscriptState {
  status: 'loading' | 'ready' | 'error';
  /** Fehlermeldung beim ersten Laden */
  message: string | null;
  entries: TranscriptEntry[];
  available: boolean;
  /** kaputte Zeilen, über alle Seiten gezählt */
  skipped: number;
  /** alle vorhandenen Seiten sind geladen (danach greift nur noch das Live-Nachladen) */
  complete: boolean;
}

const INITIAL: TranscriptState = { status: 'loading', message: null, entries: [], available: true, skipped: 0, complete: false };

// Hängt neue Einträge an; kommt ein Eintrag mit bekannter id erneut (z. B. ein Tool-Aufruf mit inzwischen
// eingetroffenem Ergebnis), ersetzt er den alten an dessen Stelle.
export function mergeEntries(current: TranscriptEntry[], incoming: TranscriptEntry[]): TranscriptEntry[] {
  if (incoming.length === 0) return current;
  const index = new Map(current.map((entry, position) => [entry.id, position]));
  const next = [...current];
  for (const entry of incoming) {
    const position = index.get(entry.id);
    if (position === undefined) {
      index.set(entry.id, next.length);
      next.push(entry);
    } else {
      next[position] = entry;
    }
  }
  return next;
}

// Laufzustand des Nachladens einer geöffneten Session; ein neuer Zustand macht laufende Anfragen des alten wirkungslos
interface PullRun {
  cursor: string | null;
  busy: boolean;
  again: boolean;
}

const freshRun = (): PullRun => ({ cursor: null, busy: false, again: false });

// Lädt Details, Verlauf (in Seiten ab Cursor) und Änderungen einer Session und hält sie live aktuell:
// Bei `session-activity` dieser Session wird der Verlauf ab dem Cursor nachgeladen, Details und Änderungen neu geholt –
// gedrosselt auf höchstens alle LIVE_THROTTLE_MS.
export function useSessionDetail(pid: string, sid: string) {
  const api = useApi();
  const detail = useAsync(() => api.getSessionDetail(pid, sid), [api, pid, sid]);
  const changes = useAsync(() => api.getSessionChanges(pid, sid), [api, pid, sid]);
  const [transcript, setTranscript] = useState<TranscriptState>(INITIAL);
  // Zählt jedes neu geladene Änderungs-Ergebnis; aufgeklappte Diffs laden damit nach
  const [changesVersion, setChangesVersion] = useState(0);
  const run = useRef<PullRun>(freshRun());

  // Holt Seiten ab dem Cursor, bis eine nicht mehr voll ist. Läuft schon ein Abruf, wird danach noch einmal geholt.
  const pull = useCallback(async () => {
    const own = run.current;
    if (own.busy) {
      own.again = true;
      return;
    }
    own.busy = true;
    try {
      do {
        own.again = false;
        for (;;) {
          const page = await api.getSessionTranscript(pid, sid, own.cursor, TRANSCRIPT_PAGE_SIZE);
          if (own !== run.current) return;
          own.cursor = page.cursor || own.cursor;
          const full = page.available && page.entries.length >= TRANSCRIPT_PAGE_SIZE;
          setTranscript((prev) => ({
            status: 'ready',
            message: null,
            entries: mergeEntries(prev.entries, page.entries),
            available: page.available,
            skipped: prev.skipped + page.skipped,
            complete: !full,
          }));
          if (!full) break;
        }
      } while (own.again);
    } catch (error) {
      if (own !== run.current) return;
      // Hat der Verlauf schon Einträge, bleiben sie stehen; das nächste Ereignis versucht es erneut
      setTranscript((prev) => (prev.status === 'ready' ? { ...prev, complete: true } : { ...INITIAL, status: 'error', message: (error as Error).message }));
    } finally {
      own.busy = false;
    }
  }, [api, pid, sid]);

  useEffect(() => {
    run.current = freshRun();
    setTranscript(INITIAL);
    void pull();
    return () => {
      run.current = freshRun();
    };
  }, [pull]);

  const retryTranscript = useCallback(() => {
    run.current = freshRun();
    setTranscript(INITIAL);
    void pull();
  }, [pull]);

  // Änderungen im Hintergrund neu holen; ein neuer Stand lässt aufgeklappte Diffs nachladen
  const { refresh: refreshChanges } = changes;
  const reloadChanges = useCallback(async () => {
    if ((await refreshChanges()) === null) setChangesVersion((n) => n + 1);
  }, [refreshChanges]);

  // Gedrosseltes Nachladen nach Live-Meldungen: sofort, wenn das letzte länger als LIVE_THROTTLE_MS her ist, sonst einmal danach
  const lastLive = useRef(0);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const { refresh: refreshDetail } = detail;
  const refreshAll = useCallback(() => {
    lastLive.current = Date.now();
    void pull();
    void refreshDetail();
    void reloadChanges();
  }, [pull, refreshDetail, reloadChanges]);
  const onActivity = useCallback(() => {
    if (timer.current) return;
    const wait = lastLive.current + LIVE_THROTTLE_MS - Date.now();
    if (wait <= 0) return refreshAll();
    timer.current = setTimeout(() => {
      timer.current = null;
      refreshAll();
    }, wait);
  }, [refreshAll]);
  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current);
      timer.current = null;
    },
    [],
  );

  useLiveHandler({
    onSessionActivity: (sessionId) => {
      if (sessionId === sid) onActivity();
    },
    onReset: refreshAll,
  });

  return {
    detail: detail.state,
    retryDetail: detail.retry,
    transcript,
    retryTranscript,
    changes: changes.state,
    retryChanges: changes.retry,
    changesVersion,
  };
}
