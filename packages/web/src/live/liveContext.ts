import { createContext } from 'react';

export type ConnectionState = 'connecting' | 'connected' | 'disconnected';

// Meldung des Servers, die der Tab statt des Windows-Toasts zeigt; `url` ist ein Pfad der Oberfläche
export interface TabNotice {
  type: 'notice';
  kind: 'bereit' | 'wartet' | 'gesammelt' | 'frage-neu' | 'recherche';
  tag: string;
  title: string;
  body: string;
  url: string;
}

export interface LiveHandler {
  /** Meldung des Servers als Browser-Benachrichtigung zeigen (nur mit ?notify=1) */
  onNotice?: (notice: TabNotice) => void;
  /** Sessions haben sich geändert */
  onSessions?: () => void;
  /** In einer Session ist etwas passiert (Hook oder gewachsenes Transcript): offene Details nachladen */
  onSessionActivity?: (sessionId: string) => void;
  /** Ein beobachtetes DB-Board hat sich geändert */
  onDbBoard?: (boardId: number) => void;
  /** Fragen eines beobachteten Board-Projekts haben sich geändert */
  onDbQuestions?: (projectId: number) => void;
  /** Dokumente (Specs, Pläne, Glossar, ADRs) im Repo des Boards haben sich geändert */
  onDocsChanged?: () => void;
  /** Der Stand ist unklar (neu verbunden oder Server meldet „reset“): alles neu laden */
  onReset?: () => void;
}

export interface LiveValue {
  state: ConnectionState;
  /** Zeitpunkt der letzten Meldung des Servers */
  lastSync: Date | null;
  reconnect: () => void;
  register: (handler: LiveHandler) => () => void;
  /** Beobachtet ein DB-Board über den Ereignisstrom (null: keins) */
  watchDbBoard: (boardId: number | null) => void;
  /** Beobachtet die Fragen eines Board-Projekts über den Ereignisstrom (mehrere Beobachter möglich); liefert die Abmeldung */
  watchDbProject: (projectId: number) => () => void;
}

// Ohne Anbieter (z. B. in Einzeltests) gibt es keine Live-Verbindung und nichts zu tun
export const LiveContext = createContext<LiveValue>({
  state: 'connecting',
  lastSync: null,
  reconnect: () => undefined,
  register: () => () => undefined,
  watchDbBoard: () => undefined,
  watchDbProject: () => () => undefined,
});
