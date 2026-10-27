import { HarnessError } from '../errors/HarnessError.js';
import type { HookEvent, Session, SessionView, Subagent } from './Session.js';

export const STALE_MS = 30 * 60 * 1000;
export const RETENTION_MS = 7 * 24 * 60 * 60 * 1000;

const WAITING_NOTIFICATIONS = new Set(['permission_prompt', 'idle_prompt']);

// Zustandsspeicher aller Sessions: wendet Hook-Ereignisse an und liefert Ansichten (reine Logik, ohne Dateien).
export class SessionStore {
  private sessions: Session[];

  constructor(
    sessions: Session[] = [],
    private readonly now: () => Date = () => new Date(),
  ) {
    this.sessions = sessions.map((s) => ({ ...s, tickets: [...s.tickets], subagents: s.subagents.map((a) => ({ ...a })) }));
  }

  // Wendet ein Ereignis an (legt unbekannte Sessions an) und bereinigt alte
  apply(projectId: string, event: HookEvent): Session {
    const at = event.at ?? this.now().toISOString();
    let session = this.sessions.find((s) => s.projectId === projectId && s.id === event.sessionId);
    if (!session) {
      session = { id: event.sessionId, projectId, tickets: [], cwd: event.cwd, state: 'idle', startedAt: at, lastEventAt: at, subagents: [] };
      this.sessions.push(session);
    }
    session.lastEventAt = at;
    session.cwd = event.cwd || session.cwd;
    if (event.title) session.title = event.title;
    if (event.model) session.model = event.model;
    for (const ticket of event.tickets ?? []) if (!session.tickets.includes(ticket)) session.tickets.push(ticket);
    this.transition(session, event, at);
    this.prune();
    return session;
  }

  // Ordnet einer bekannten Session ein Ticket zu
  claim(projectId: string, sessionId: string, ticketId: string): Session {
    const session = this.sessions.find((s) => s.projectId === projectId && s.id === sessionId);
    if (!session) throw new HarnessError('session-not-found', `Session ${sessionId} gibt es nicht.`);
    if (!session.tickets.includes(ticketId)) session.tickets.push(ticketId);
    return session;
  }

  // Ansichten, neueste Aktivität zuerst; optional nur ein Projekt
  list(projectId?: string): SessionView[] {
    return this.sessions
      .filter((s) => projectId === undefined || s.projectId === projectId)
      .map((s) => this.view(s))
      .sort((a, b) => b.lastEventAt.localeCompare(a.lastEventAt));
  }

  // Sessions, die einem Ticket zugeordnet sind
  forTicket(projectId: string, ticketId: string): SessionView[] {
    return this.list(projectId).filter((s) => s.tickets.includes(ticketId));
  }

  // Entfernt Sessions, deren letzte Meldung länger als 7 Tage zurückliegt; liefert die Anzahl
  prune(): number {
    const limit = this.now().getTime() - RETENTION_MS;
    const before = this.sessions.length;
    this.sessions = this.sessions.filter((s) => new Date(s.endedAt ?? s.lastEventAt).getTime() >= limit);
    return before - this.sessions.length;
  }

  // Alle Sessions zum Speichern
  toJSON(): Session[] {
    return this.sessions;
  }

  // Zustandsübergang je Ereignis (siehe data-model.md)
  private transition(session: Session, event: HookEvent, at: string): void {
    switch (event.kind) {
      case 'session-start':
        session.state = 'idle';
        delete session.endedAt;
        break;
      case 'user-prompt':
        session.state = 'running';
        break;
      case 'notification':
        if (WAITING_NOTIFICATIONS.has(event.notificationType ?? '') && !this.isIdleWhileSubagentsRun(session, event)) session.state = 'waiting';
        break;
      case 'stop':
        session.state = 'idle';
        this.endSubagents(session, at);
        break;
      case 'subagent-start':
        session.state = 'running';
        this.upsertSubagent(session, event, 'running', at);
        break;
      case 'subagent-stop':
        this.upsertSubagent(session, event, 'ended', at);
        break;
      case 'session-end':
        session.state = 'ended';
        session.endedAt = at;
        this.endSubagents(session, at);
        break;
    }
  }

  // Der Leerlauf-Hinweis ist kein Warten auf den Benutzer, solange Subagenten arbeiten
  private isIdleWhileSubagentsRun(session: Session, event: HookEvent): boolean {
    return event.notificationType === 'idle_prompt' && session.subagents.some((a) => a.state === 'running');
  }

  // Legt einen Subagenten an oder setzt seinen Zustand
  private upsertSubagent(session: Session, event: HookEvent, state: Subagent['state'], at: string): void {
    const id = event.agentId ?? `${event.agentType ?? 'agent'}-${session.subagents.length + 1}`;
    let agent = session.subagents.find((a) => a.id === id);
    if (!agent) {
      agent = { id, type: event.agentType ?? 'unbekannt', state, startedAt: at };
      session.subagents.push(agent);
    }
    agent.state = state;
    if (state === 'ended') agent.endedAt = at;
  }

  // Beendet alle noch laufenden Subagenten
  private endSubagents(session: Session, at: string): void {
    for (const agent of session.subagents) {
      if (agent.state === 'running') {
        agent.state = 'ended';
        agent.endedAt = at;
      }
    }
  }

  // Berechnet Anzeigezustand, Lauschen und Laufzeit
  private view(session: Session): SessionView {
    const now = this.now().getTime();
    const stale = session.state !== 'ended' && now - new Date(session.lastEventAt).getTime() > STALE_MS;
    const displayState = stale ? 'stale' : session.state;
    const end = session.endedAt ? new Date(session.endedAt).getTime() : now;
    return {
      ...session,
      tickets: [...session.tickets],
      subagents: session.subagents.map((a) => ({ ...a })),
      displayState,
      listening: displayState === 'idle' || displayState === 'running' || displayState === 'waiting',
      runtimeMs: Math.max(0, end - new Date(session.startedAt).getTime()),
    };
  }
}
