import { existsSync } from 'node:fs';
import { ResumeCommand, type Favorite, type SessionState, type SessionView } from '@harness/core';
import type { TicketInfo } from '../sessions/SessionDtoMapper.js';

export interface FavoriteDto {
  sessionId: string;
  note: string;
  tickets: string[];
  ticketInfo: TicketInfo[];
  cwd: string;
  cwdExists: boolean;
  title: string | null;
  markedAt: string;
  state: SessionState;
  displayState: string;
  live: boolean;
  lastEventAt: string;
  resumedAs: string[];
  command: string;
}

// Bildet Favoriten auf das Antwortformat ab; ist die Session noch bekannt, gilt ihr aktueller Zustand.
export class FavoriteDtoMapper {
  // Ein Favorit mit optionaler Live-Session und Ticketangaben
  toDto(favorite: Favorite, live: SessionView | undefined, info: Map<string, TicketInfo>): FavoriteDto {
    const tickets = live?.tickets ?? favorite.tickets;
    return {
      sessionId: favorite.sessionId,
      note: favorite.note,
      tickets,
      ticketInfo: tickets.map((id) => info.get(id) ?? { id, title: '(unbekannt)', status: 'backlog' }),
      cwd: favorite.cwd,
      cwdExists: existsSync(favorite.cwd),
      title: live?.title ?? favorite.title ?? null,
      markedAt: favorite.markedAt,
      state: live?.state ?? favorite.lastState,
      displayState: live?.displayState ?? favorite.lastState,
      live: live !== undefined,
      lastEventAt: live?.lastEventAt ?? favorite.lastEventAt,
      resumedAs: favorite.resumedAs,
      command: ResumeCommand.build(favorite.cwd, favorite.sessionId),
    };
  }
}
