import type { DisplayState, SessionState, SessionView } from '@harness/core';

export interface TicketInfo {
  id: string;
  title: string;
  status: string;
}

export interface SessionDto {
  id: string;
  projectId: string;
  tickets: string[];
  ticketInfo: TicketInfo[];
  cwd: string;
  state: SessionState;
  displayState: DisplayState;
  listening: boolean;
  startedAt: string;
  lastEventAt: string;
  endedAt: string | null;
  runtimeMs: number;
  title: string | null;
  model: string | null;
  subagents: Array<{ id: string; type: string; state: 'running' | 'ended'; startedAt: string; endedAt: string | null }>;
}

// Bildet Session-Ansichten auf das Antwortformat ab (fehlende Angaben als null).
export class SessionDtoMapper {
  // Eine Session
  toDto(view: SessionView, info: Map<string, TicketInfo> = new Map()): SessionDto {
    return {
      id: view.id,
      projectId: view.projectId,
      tickets: view.tickets,
      ticketInfo: view.tickets.map((id) => info.get(id) ?? { id, title: '(unbekannt)', status: 'backlog' }),
      cwd: view.cwd,
      state: view.state,
      displayState: view.displayState,
      listening: view.listening,
      startedAt: view.startedAt,
      lastEventAt: view.lastEventAt,
      endedAt: view.endedAt ?? null,
      runtimeMs: view.runtimeMs,
      title: view.title ?? null,
      model: view.model ?? null,
      subagents: view.subagents.map((a) => ({ ...a, endedAt: a.endedAt ?? null })),
    };
  }
}
