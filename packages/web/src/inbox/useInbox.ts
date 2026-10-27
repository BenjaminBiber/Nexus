import { useCallback, useEffect, useRef, useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import { ApiError } from '../api/ApiError.js';
import type { InboxListDto } from '../api/inboxTypes.js';
import { useAsync } from '../hooks/useAsync.js';
import { useLiveHandler } from '../live/useLive.js';

// Board-Ereignisse kommen nur für das gerade beobachtete Board an, nicht für alle eigenen Boards: daher lädt die Liste
// zusätzlich in diesem Abstand nach, solange die Seite sichtbar ist
export const INBOX_POLL_MS = 30_000;

// „Meine Aufgaben“ (alle eigenen Boards oder ein Board). Eine nicht verfügbare Board-Datenbank (503) steht in `unavailable`.
// Live: Fragen-Meldungen (Projekte beobachtet die Seite), neue Verbindung und Abfrage alle 30 s im Hintergrund;
// scheitert das Nachladen, bleibt die Liste stehen und `notice` nennt den Fehler.
export function useInbox(boardId: number | null, poll = INBOX_POLL_MS) {
  const api = useApi();
  const [unavailable, setUnavailable] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const background = useRef(false);
  const loaded = useAsync<InboxListDto>(async () => {
    const quiet = background.current;
    try {
      const data = await api.getInbox(boardId);
      setUnavailable(null);
      setNotice(null);
      return data;
    } catch (error) {
      if (!quiet && error instanceof ApiError && error.status === 503) setUnavailable(error.message);
      throw error;
    }
  }, [api, boardId]);
  const { refresh } = loaded;
  const liveReload = useCallback(async () => {
    background.current = true;
    const pending = refresh();
    background.current = false;
    const message = await pending;
    if (message) setNotice(`Aktueller Stand konnte nicht geladen werden: ${message}`);
  }, [refresh]);
  useLiveHandler({ onDbQuestions: () => void liveReload(), onDbBoard: () => void liveReload(), onReset: () => void liveReload() });
  useEffect(() => {
    if (poll <= 0) return undefined;
    const timer = setInterval(() => {
      if (document.visibilityState === 'visible') void liveReload();
    }, poll);
    return () => clearInterval(timer);
  }, [poll, liveReload]);
  return { state: loaded.state, reload: loaded.reload, retry: loaded.retry, set: loaded.set, unavailable, notice };
}
