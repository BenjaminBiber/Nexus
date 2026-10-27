import { HarnessError } from '../errors/HarnessError.js';
import type { BoardDatabase } from './BoardDatabase.js';
import type { BoardActor, Comment } from './BoardTypes.js';
import { BoardLimits } from './BoardLimits.js';
import { TicketService } from './TicketService.js';

// Kommentare an Tickets: nur anhängen, nie ändern. Ein Kommentar erhöht die Version des Tickets.
export class CommentService {
  private readonly tickets: TicketService;

  constructor(private readonly db: BoardDatabase, private readonly clock: () => Date) {
    this.tickets = new TicketService(db, clock);
  }

  // Kommentare dieser Tickets, die noch in keine Session eingespeist wurden
  unreadForAgent(ticketIds: string[]): Promise<Comment[]> {
    if (!ticketIds.length) return Promise.resolve([]);
    return this.db.transaction((u) => u.comments.unreadByAgent(ticketIds));
  }

  // Markiert Kommentare als eingespeist (ohne neue Version des Tickets)
  async markReadByAgent(ids: number[]): Promise<void> {
    if (!ids.length) return;
    await this.db.transaction((u) => u.comments.markReadByAgent(ids, this.clock().toISOString()));
  }

  // Hängt einen Kommentar an; readByAgent: aus einer Session geschrieben, gilt sofort als eingespeist
  add(ticketId: string, body: string, actor: BoardActor, options: { readByAgent?: boolean } = {}): Promise<Comment> {
    const text = body.trim();
    if (!text) return Promise.reject(new HarnessError('comment-empty', 'Der Kommentar darf nicht leer sein.'));
    return this.db.transaction(async (u) => {
      BoardLimits.actor(actor);
      const ticket = await this.tickets.load(u, ticketId, undefined, true);
      const at = this.clock().toISOString();
      await u.tickets.update({ ...ticket, rev: ticket.rev + 1, updatedAt: at });
      return u.comments.insert({
        ticketId: ticket.id, authorId: actor.userId, authorName: actor.name, body: text, createdAt: at,
        readByAgentAt: options.readByAgent ? at : null,
      });
    });
  }

  // Alle Kommentare eines Tickets in zeitlicher Folge
  list(ticketId: string): Promise<Comment[]> {
    return this.db.transaction(async (u) => u.comments.list((await this.tickets.load(u, ticketId)).id));
  }
}
