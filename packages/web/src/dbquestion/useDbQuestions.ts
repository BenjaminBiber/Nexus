import { useCallback, useRef, useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import { ApiError } from '../api/ApiError.js';
import type { DbQuestionListDto } from '../api/dbTypes.js';
import { useAsync } from '../hooks/useAsync.js';
import { useDbProjectWatch, useLiveHandler } from '../live/useLive.js';

// Fragen eines Board-Projekts; ohne Projekt null. Eine nicht verfügbare Board-Datenbank (503) steht in `unavailable`.
// Live: beobachtet das Projekt und lädt bei „dbquestions“ dieses Projekts oder nach einer neuen Verbindung im Hintergrund nach;
// scheitert das (auch mit 503), bleibt die Liste stehen und `notice` nennt den Fehler.
export function useDbQuestions(projectKey: string | null, projectId: number | null) {
  const api = useApi();
  const [unavailable, setUnavailable] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  // Gesetzt, solange ein Nachladen im Hintergrund startet: dessen 503 ersetzt die Liste nicht
  const background = useRef(false);
  const loaded = useAsync<DbQuestionListDto | null>(async () => {
    const quiet = background.current;
    if (!projectKey) return null;
    try {
      const data = await api.getDbQuestions(projectKey);
      setUnavailable(null);
      setNotice(null);
      return data;
    } catch (error) {
      if (!quiet && error instanceof ApiError && error.status === 503) setUnavailable(error.message);
      throw error;
    }
  }, [api, projectKey]);
  const { refresh } = loaded;
  // Lädt im Hintergrund nach; bei einem Fehler bleibt die Liste und der Hinweis erscheint
  const liveReload = useCallback(async () => {
    background.current = true;
    const pending = refresh();
    background.current = false;
    const message = await pending;
    if (message) setNotice(`Aktueller Stand konnte nicht geladen werden: ${message}`);
  }, [refresh]);
  useDbProjectWatch(projectId);
  useLiveHandler({
    onDbQuestions: (id) => {
      if (id === projectId) void liveReload();
    },
    onReset: () => void liveReload(),
  });
  return { state: loaded.state, reload: loaded.reload, retry: loaded.retry, unavailable, notice };
}
