import { useCallback, useEffect, useRef, useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import type { DispatchRunDto } from '../api/dispatchTypes.js';

// Zustand von „Recherche starten“ für eine Frage: läuft eine Recherche des Dispatchers, Pause, Start, Fehler. Solange eine
// Recherche läuft, wird alle `intervalMs` nachgefragt und `onRefresh` lädt die Frage nach – auch einmal nach dem Ende
// (die Recherche kommt außerdem live über „dbquestions“). Beim Wechsel der Frage beginnt der Zustand neu; Antworten zur
// vorigen Frage werden verworfen.
export function useQuestionResearch(questionId: string | undefined, onRefresh: () => void, intervalMs = 10000) {
  const api = useApi();
  const [run, setRun] = useState<DispatchRunDto | null>(null);
  const [pausedUntil, setPausedUntil] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const refresh = useRef(onRefresh);
  refresh.current = onRefresh;
  const wasRunning = useRef(false);
  const current = useRef(questionId);

  // Andere Frage: alles zurücksetzen (vor dem Nachladen unten)
  useEffect(() => {
    current.current = questionId;
    wasRunning.current = false;
    setRun(null);
    setPausedUntil(null);
    setError(null);
    setBusy(false);
  }, [questionId]);

  // Fragt den Stand ab; ein Fehler (z. B. älterer Server) lässt den Knopf trotzdem bedienbar
  const reload = useCallback(async () => {
    if (!questionId) return;
    try {
      const status = await api.getQuestionResearch(questionId);
      if (current.current !== questionId) return;
      setRun(status.run);
      setPausedUntil(status.pausedUntil);
      if (wasRunning.current) refresh.current();
      wasRunning.current = status.run !== null;
    } catch {
      /* Stand unbekannt – der Start meldet Fehler selbst */
    }
  }, [api, questionId]);

  useEffect(() => {
    void reload();
  }, [reload]);

  useEffect(() => {
    if (!run) return undefined;
    const timer = setInterval(() => void reload(), intervalMs);
    return () => clearInterval(timer);
  }, [run, reload, intervalMs]);

  // Startet die Recherche; Fehler erscheinen am Knopf
  const start = useCallback(async () => {
    if (!questionId) return;
    setBusy(true);
    setError(null);
    try {
      const result = await api.startQuestionResearch(questionId);
      if (current.current !== questionId) return;
      setRun(result.started);
      setPausedUntil(null);
      wasRunning.current = true;
    } catch (e) {
      if (current.current === questionId) setError((e as Error).message);
    } finally {
      if (current.current === questionId) setBusy(false);
    }
  }, [api, questionId]);

  return { run, pausedUntil, error, busy, start };
}
