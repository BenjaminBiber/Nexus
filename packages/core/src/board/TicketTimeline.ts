// Zeitleiste eines Tickets: Ereignisse aus Verlauf, Kommentaren, Fragen und (im Server) lokalen Quellen.

export const TIMELINE_KINDS = ['status', 'claim', 'run', 'question', 'git', 'comment', 'content', 'other'] as const;
export type TimelineKind = (typeof TIMELINE_KINDS)[number];

// Wer gehandelt hat; type ist eine Einschätzung am Namen bzw. an der Art des Autors
export interface TimelineActor {
  name: string;
  type: 'person' | 'agent';
}

// Verweise eines Ereignisses (Frage, Commit, Lauf-Protokoll, Profil, Exit-Code)
export interface TimelineRef {
  questionId?: string;
  commit?: string;
  runLog?: string;
  profile?: string;
  exitCode?: number | null;
}

export interface TimelineEvent {
  at: string;
  kind: TimelineKind;
  // Genauer: 'field:status', 'comment', 'dispatch:start', 'run-log', 'commit', 'merge', 'question:decided' …
  type: string;
  actor: TimelineActor;
  field?: string;
  from?: string | null;
  to?: string | null;
  note?: string | null;
  text?: string;
  ref?: TimelineRef;
}

// Namen, an denen Agenten erkannt werden (aus der Dispatch-Konfiguration)
export interface TimelineAgentNames {
  dispatcherName: string;
  agentPrefix: string;
}

export interface TicketTimeline {
  ticketId: string;
  events: TimelineEvent[];
}

// Sortiert aufsteigend nach Zeit; gleiche Zeit behält die Reihenfolge der Eingabe (stabil)
export function sortTimeline(events: TimelineEvent[]): TimelineEvent[] {
  return events.map((e, i) => ({ e, i })).sort((a, b) => a.e.at.localeCompare(b.e.at) || a.i - b.i).map((x) => x.e);
}
