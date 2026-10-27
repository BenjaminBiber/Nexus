import type { SessionState } from '../sessions/Session.js';

// Gemerkte Session eines Projekts: Notiz plus Momentaufnahme, damit sie auch nach der 7-Tage-Frist erhalten bleibt.
export interface Favorite {
  projectId: string;
  sessionId: string;
  note: string;
  tickets: string[];
  cwd: string;
  title?: string;
  markedAt: string;
  lastState: SessionState;
  lastEventAt: string;
  /** Kennungen fortgesetzter Sessions, die diesem Favoriten zugeordnet wurden */
  resumedAs: string[];
}

// Momentaufnahme einer Session beim Markieren oder Aktualisieren
export interface SessionSnapshot {
  sessionId: string;
  tickets: string[];
  cwd: string;
  title?: string;
  state: SessionState;
  lastEventAt: string;
}
