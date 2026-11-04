import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { useApi } from '../api/ApiContext.js';
import { usePlatform } from '../platform/PlatformContext.js';
import { LiveContext, type ConnectionState, type LiveHandler, type TabNotice } from './liveContext.js';

// Erste Antwort auf „darf dieser Tab Meldungen zeigen?“, noch ohne die Plattform zu fragen.
// So verbindet ein Browser mit erteilter Erlaubnis sofort richtig; die Desktop-App holt das gleich danach nach.
function notifyAllowedAtOnce(): boolean {
  return typeof Notification !== 'undefined' && Notification.permission === 'granted';
}

// Hält die Verbindung zum Ereignisstrom des Projekts, verteilt Meldungen und merkt sich den Verbindungszustand.
// Ein beobachtetes DB-Board und ein beobachtetes Board-Projekt (Fragen) werden beim Öffnen genannt; ein Wechsel verbindet neu.
// Board-Projekte zählen mehrere Beobachter: der Server kennt nur eins, es gilt das des zuletzt hinzugekommenen verbliebenen.
// Gehen alle Beobachter, bleibt das Projekt noch `releaseDelayMs` beobachtet (kein Neuverbinden beim Wechsel Liste → Detail).
export function LiveProvider({ pid, children, releaseDelayMs = 1000 }: { pid: string; children: ReactNode; releaseDelayMs?: number }) {
  const api = useApi();
  const platform = usePlatform();
  const [state, setState] = useState<ConnectionState>('connecting');
  const [notifyAllowed, setNotifyAllowed] = useState(notifyAllowedAtOnce);
  const [lastSync, setLastSync] = useState<Date | null>(null);
  const [attempt, setAttempt] = useState(0);
  const [dbBoard, setDbBoard] = useState<number | null>(null);
  const [dbProject, setDbProject] = useState<number | null>(null);
  const handlers = useRef(new Set<LiveHandler>());
  const projectWatchers = useRef<{ projectId: number }[]>([]);
  const releaseTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const releaseDelay = useRef(releaseDelayMs);
  releaseDelay.current = releaseDelayMs;
  useEffect(() => () => {
    if (releaseTimer.current) clearTimeout(releaseTimer.current);
  }, []);

  // Die Erlaubnis kommt von der Plattform und damit erst später; ein Wechsel verbindet den Strom neu.
  // Nach jedem `reconnect()` wird sie erneut gelesen, denn danach kann sie gerade erteilt worden sein.
  useEffect(() => {
    let alive = true;
    platform
      .notificationPermission()
      .then((permission) => alive && setNotifyAllowed(permission === 'granted'))
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [platform, attempt]);

  useEffect(() => {
    let wasDown = false;
    setState('connecting');
    // Ohne beobachtetes Projekt wie bisher mit zwei Argumenten; mit erlaubten Benachrichtigungen zeigt der Tab die Meldungen des Servers
    const source = notifyAllowed
      ? api.openEvents(pid, dbBoard, dbProject, true)
      : dbProject === null ? api.openEvents(pid, dbBoard) : api.openEvents(pid, dbBoard, dbProject);
    source.onmessage = (event) => {
      const message = JSON.parse(event.data) as { type: string; boardId?: number; projectId?: number; sessionId?: string };
      if (message.type === 'notice') return handlers.current.forEach((h) => h.onNotice?.(message as unknown as TabNotice));
      setLastSync(new Date());
      if (message.type === 'hello') {
        setState('connected');
        if (wasDown) handlers.current.forEach((h) => h.onReset?.());
        wasDown = false;
      } else if (message.type === 'sessions') {
        handlers.current.forEach((h) => h.onSessions?.());
      } else if (message.type === 'session-activity' && typeof message.sessionId === 'string') {
        handlers.current.forEach((h) => h.onSessionActivity?.(message.sessionId!));
      } else if (message.type === 'dbboard' && typeof message.boardId === 'number') {
        handlers.current.forEach((h) => h.onDbBoard?.(message.boardId!));
      } else if (message.type === 'dbquestions' && typeof message.projectId === 'number') {
        handlers.current.forEach((h) => h.onDbQuestions?.(message.projectId!));
      } else if (message.type === 'docs-changed') {
        handlers.current.forEach((h) => h.onDocsChanged?.());
      } else if (message.type === 'reset') {
        handlers.current.forEach((h) => h.onReset?.());
      }
    };
    source.onerror = () => {
      wasDown = true;
      setState('disconnected');
    };
    return () => source.close();
  }, [api, pid, attempt, dbBoard, dbProject, notifyAllowed]);

  const register = useCallback((handler: LiveHandler) => {
    handlers.current.add(handler);
    return () => void handlers.current.delete(handler);
  }, []);
  const reconnect = useCallback(() => setAttempt((n) => n + 1), []);
  const watchDbBoard = useCallback((boardId: number | null) => setDbBoard(boardId), []);
  // Meldet einen Beobachter eines Board-Projekts an; liefert die Abmeldung
  const watchDbProject = useCallback((projectId: number) => {
    const entry = { projectId };
    projectWatchers.current.push(entry);
    const apply = () => {
      if (releaseTimer.current) clearTimeout(releaseTimer.current);
      releaseTimer.current = null;
      const last = projectWatchers.current.at(-1);
      if (last) setDbProject(last.projectId);
      else releaseTimer.current = setTimeout(() => {
        releaseTimer.current = null;
        if (!projectWatchers.current.length) setDbProject(null);
      }, releaseDelay.current);
    };
    apply();
    return () => {
      const index = projectWatchers.current.indexOf(entry);
      if (index < 0) return;
      projectWatchers.current.splice(index, 1);
      apply();
    };
  }, []);
  const value = useMemo(
    () => ({ state, lastSync, reconnect, register, watchDbBoard, watchDbProject }),
    [state, lastSync, reconnect, register, watchDbBoard, watchDbProject],
  );

  return <LiveContext.Provider value={value}>{children}</LiveContext.Provider>;
}
