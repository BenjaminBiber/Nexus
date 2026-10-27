import { useCallback, useEffect, useRef, useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import type { DispatchRunDto } from '../api/dispatchTypes.js';

// Zustand von „Planen“ für ein Ticket: läuft eine Planung des Dispatchers, Pause, Start, Fehler. Solange eine Planung läuft,
// wird alle `intervalMs` nachgefragt; nach dem Ende lädt `onRefresh` das Ticket nach (Plan, Status, Kommentar). Beim Wechsel
// des Tickets beginnt der Zustand neu; Antworten zum vorigen Ticket werden verworfen.
export function useTicketPlanning(ticketId: string | undefined, boardId: number | undefined, onRefresh: () => void, intervalMs = 10000) {
  const api = useApi();
  const [run, setRun] = useState<DispatchRunDto | null>(null);
  const [pausedUntil, setPausedUntil] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const refresh = useRef(onRefresh);
  refresh.current = onRefresh;
  const wasRunning = useRef(false);
  const current = useRef(ticketId);

  // Anderes Ticket: alles zurücksetzen (vor dem Nachladen unten)
  useEffect(() => {
    current.current = ticketId;
    wasRunning.current = false;
    setRun(null);
    setPausedUntil(null);
    setError(null);
    setBusy(false);
  }, [ticketId]);

  // Fragt den Stand ab; ein Fehler (z. B. älterer Server) lässt den Knopf trotzdem bedienbar
  const reload = useCallback(async () => {
    if (!ticketId) return;
    try {
      const status = await api.getTicketPlanning(ticketId);
      if (current.current !== ticketId) return;
      setRun(status.run);
      setPausedUntil(status.pausedUntil);
      if (wasRunning.current && status.run === null) refresh.current();
      wasRunning.current = status.run !== null;
    } catch {
      /* Stand unbekannt – der Start meldet Fehler selbst */
    }
  }, [api, ticketId]);

  useEffect(() => {
    void reload();
  }, [reload]);

  useEffect(() => {
    if (!run) return undefined;
    const timer = setInterval(() => void reload(), intervalMs);
    return () => clearInterval(timer);
  }, [run, reload, intervalMs]);

  // Startet die Planung; Fehler erscheinen am Knopf
  const start = useCallback(async () => {
    if (!ticketId) return;
    setBusy(true);
    setError(null);
    try {
      const result = await api.startTicketPlanning(ticketId, boardId);
      if (current.current !== ticketId) return;
      setRun(result.started);
      setPausedUntil(null);
      wasRunning.current = true;
      refresh.current();
    } catch (e) {
      if (current.current === ticketId) setError((e as Error).message);
    } finally {
      if (current.current === ticketId) setBusy(false);
    }
  }, [api, ticketId, boardId]);

  return { run, pausedUntil, error, busy, start };
}
