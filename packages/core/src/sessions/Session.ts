export type SessionState = 'idle' | 'running' | 'waiting' | 'ended';
export type DisplayState = SessionState | 'stale';

export interface Subagent {
  id: string;
  type: string;
  state: 'running' | 'ended';
  startedAt: string;
  endedAt?: string;
}

// Ein Claude-Lauf mit Zuordnung zu Tickets und seinen Subagenten.
export interface Session {
  id: string;
  projectId: string;
  tickets: string[];
  cwd: string;
  state: SessionState;
  startedAt: string;
  lastEventAt: string;
  endedAt?: string;
  title?: string;
  model?: string;
  subagents: Subagent[];
}

// Beim Lesen berechnete Angaben: Anzeigezustand, Lauschen und Laufzeit.
export interface SessionView extends Session {
  displayState: DisplayState;
  listening: boolean;
  runtimeMs: number;
}

export type HookKind =
  | 'session-start'
  | 'user-prompt'
  | 'notification'
  | 'stop'
  | 'subagent-start'
  | 'subagent-stop'
  | 'session-end';

export const HOOK_KINDS: HookKind[] = ['session-start', 'user-prompt', 'notification', 'stop', 'subagent-start', 'subagent-stop', 'session-end'];

// Normalisiertes Ereignis eines Hooks (CLI → Server).
export interface HookEvent {
  kind: HookKind;
  sessionId: string;
  cwd: string;
  at?: string;
  tickets?: string[];
  agentId?: string;
  agentType?: string;
  notificationType?: string;
  title?: string;
  model?: string;
  /** Quelle des Sessionstarts bei Claude Code, z. B. startup oder resume */
  source?: string;
}
