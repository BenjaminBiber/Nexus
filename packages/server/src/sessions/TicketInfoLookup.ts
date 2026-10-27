import type { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import type { TicketInfo } from './SessionDtoMapper.js';

const DEFAULT_TIMEOUT_MS = 1000;

// Liest Titel und Status von Tickets aus der Board-DB für Sessions, Favoriten und Meldungen.
export class TicketInfoLookup {
  constructor(
    private readonly holder: Pick<BoardRuntimeHolder, 'run'>,
    private readonly timeoutMs = DEFAULT_TIMEOUT_MS,
  ) {}

  // Angaben je Ticket-ID; ohne eingerichtete, erreichbare oder rechtzeitig antwortende Board-DB eine leere Zuordnung (nie ein Fehler)
  async infos(ids: string[]): Promise<Map<string, TicketInfo>> {
    const wanted = [...new Set(ids)];
    if (!wanted.length) return new Map();
    let timer: ReturnType<typeof setTimeout> | undefined;
    const timeout = new Promise<never>((_, reject) => {
      timer = setTimeout(() => reject(new Error('board-db-timeout')), this.timeoutMs);
    });
    try {
      const list = await Promise.race([this.holder.run((b) => b.services.tickets.summaries(wanted)), timeout]);
      return new Map(list.map((t) => [t.id, { id: t.id, title: t.title, status: t.status }]));
    } catch {
      return new Map();
    } finally {
      clearTimeout(timer);
    }
  }
}
