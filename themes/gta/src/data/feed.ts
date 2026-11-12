// Hält den Board-Stand aktuell: sofort laden, dann über den Ereignisstrom des Servers bei Änderungen nachladen. Ein
// Sicherheitsnetz lädt alle 30 s, solange der Strom nicht offen ist. Beides nur im sichtbaren Tab; wird er wieder
// sichtbar, wird einmal nachgeladen. Fehler bleiben still; der letzte gute Stand bleibt.
import { useEffect, useState } from 'react';
import { eventsUrl, fetchBoardView } from './api.js';
import type { BoardView } from './types.js';

export interface BoardFeed {
  view: BoardView | null;
  failed: boolean;
}

export const SAFETY_MS = 30_000;
/** Länger wartet eine Anfrage nicht; danach gilt sie als gescheitert und blockiert kein Nachladen mehr */
export const REQUEST_MS = 20_000;
const OPEN = 1; // EventSource.OPEN

const EMPTY: BoardFeed = { view: null, failed: false };

// Strom des Boards oder null, wenn es keinen EventSource gibt. Nach einer Unterbrechung kann eine Änderung verpasst
// worden sein; deshalb beim Wiederverbinden einmal nachladen.
function openStream(pid: string, boardId: number, onChange: () => void): EventSource | null {
  if (typeof EventSource === 'undefined') return null;
  const source = new EventSource(eventsUrl(pid, boardId));
  let wasDown = false;
  source.onerror = () => {
    wasDown = true;
  };
  source.onopen = () => {
    if (!wasDown) return;
    wasDown = false;
    onChange();
  };
  source.onmessage = (event: MessageEvent) => {
    let message: { type?: unknown; boardId?: unknown };
    try {
      message = JSON.parse(String(event.data));
    } catch {
      return;
    }
    if ((message.type === 'dbboard' && message.boardId === boardId) || message.type === 'reset') onChange();
  };
  return source;
}

export function useBoardFeed(pid: string, boardId: number | null): BoardFeed {
  const key = boardId === null ? null : `${pid}:${boardId}`;
  // Stand samt Board, zu dem er gehört: nach einem Wechsel gilt er sofort nicht mehr
  const [state, setState] = useState<{ key: string | null; feed: BoardFeed }>({ key: null, feed: EMPTY });

  useEffect(() => {
    if (boardId === null) return;
    // Alten Stand verwerfen: ein Rückwechsel A → B → A zeigt sonst kurz den alten Stand von A
    setState({ key, feed: EMPTY });
    let alive = true;
    let running: AbortController | null = null;
    let timeout: ReturnType<typeof setTimeout> | undefined;
    let again = false;

    const load = () => {
      if (running) {
        again = true;
        return;
      }
      const controller = new AbortController();
      running = controller;
      timeout = setTimeout(() => controller.abort(), REQUEST_MS);
      void fetchBoardView(boardId, controller.signal).then((view) => {
        clearTimeout(timeout);
        if (!alive) return;
        running = null;
        setState((prev) => {
          const last = prev.key === key ? prev.feed.view : null;
          return { key, feed: view ? { view, failed: false } : { view: last, failed: true } };
        });
        if (again) {
          again = false;
          load();
        }
      });
    };

    // Strom und Sicherheitsnetz nur im sichtbaren Tab: Über http:// erlaubt der Browser 6 Verbindungen je Herkunft, und
    // der Kern hält je Tab schon einen eigenen Strom offen – mit unserem zweiten hingen ab drei Tabs weitere Anfragen.
    let source: EventSource | null = null;
    let safety: ReturnType<typeof setInterval> | undefined;
    const connect = () => {
      source = openStream(pid, boardId, load);
      safety = setInterval(() => {
        if (!source || source.readyState !== OPEN) load();
      }, SAFETY_MS);
    };
    const disconnect = () => {
      source?.close();
      source = null;
      clearInterval(safety);
      safety = undefined;
    };
    const onVisibility = () => {
      const visible = document.visibilityState !== 'hidden';
      if (!visible) disconnect();
      else if (safety === undefined) {
        connect();
        load();
      }
    };

    load();
    if (document.visibilityState !== 'hidden') connect();
    document.addEventListener('visibilitychange', onVisibility);

    return () => {
      alive = false;
      document.removeEventListener('visibilitychange', onVisibility);
      disconnect();
      clearTimeout(timeout);
      running?.abort();
    };
  }, [pid, boardId, key]);

  return key !== null && state.key === key ? state.feed : EMPTY;
}
