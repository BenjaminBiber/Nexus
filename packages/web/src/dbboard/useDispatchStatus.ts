import { useCallback, useEffect, useRef, useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import type { DispatchStatusDto } from '../api/dispatchTypes.js';

// Zustand des Dispatchers für ein Board; fragt regelmäßig nach (Standard alle 15 s), Fehler bleiben still sichtbar.
export function useDispatchStatus(boardId: number | null, intervalMs = 15000) {
  const api = useApi();
  const [status, setStatus] = useState<DispatchStatusDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const latest = useRef(0);

  const reload = useCallback(async () => {
    const id = ++latest.current;
    try {
      const next = await api.getDispatchStatus(boardId ?? undefined);
      if (id === latest.current) {
        setStatus(next);
        setError(null);
      }
    } catch (e) {
      if (id === latest.current) setError((e as Error).message);
    }
  }, [api, boardId]);

  useEffect(() => {
    setStatus(null);
    setError(null);
    void reload();
    const timer = setInterval(() => void reload(), intervalMs);
    return () => clearInterval(timer);
  }, [reload, intervalMs]);

  // Setzt eine Antwort der Steuerung direkt (verwirft laufende Abfragen)
  const set = useCallback((next: DispatchStatusDto) => {
    latest.current++;
    setStatus(next);
    setError(null);
  }, []);

  return { status, error, reload, set };
}
