import type { SessionDto } from '../api/types.js';

export type KnownStates = Map<string, SessionDto['displayState']>;

// Sessions, die seit dem letzten Stand bereit sind oder auf Eingabe warten (nicht beim ersten Laden)
export function findReadySessions(known: KnownStates, sessions: SessionDto[]): SessionDto[] {
  if (known.size === 0) return [];
  return sessions.filter((s) => {
    const before = known.get(s.id);
    if (before === s.displayState) return false;
    if (s.displayState === 'waiting') return true;
    return s.displayState === 'idle' && before === 'running';
  });
}

// Merkt sich den aktuellen Zustand aller Sessions
export function rememberStates(sessions: SessionDto[]): KnownStates {
  return new Map(sessions.map((s) => [s.id, s.displayState]));
}

// Ziel beim Klick auf die Benachrichtigung: erstes Ticket im Board, sonst die Sessions des Projekts (wie die Windows-Meldung)
export function readyTarget(pid: string, session: SessionDto): string {
  const ticket = session.tickets[0];
  return ticket ? `/p/${pid}/board/t/${encodeURIComponent(ticket)}` : `/p/${pid}/sessions`;
}

// Titel und Text der Benachrichtigung
export function describeReady(session: SessionDto): { title: string; body: string } {
  const title = session.displayState === 'waiting' ? 'Session wartet auf Eingabe' : 'Session ist bereit';
  const body = session.title ?? (session.tickets.join(', ') || session.cwd);
  return { title, body };
}
