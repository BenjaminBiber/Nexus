import type { BoardUnitOfWork } from './BoardDatabase.js';
import type { BoardOpenQuestion } from './BoardQuestionTypes.js';

// Ab so vielen Tickets werden alle offenen Fragen geladen statt nach Tickets gefiltert (Grenze der SQL-Parameter)
const MANY_TICKETS = 500;

// Ermittelt die offenen Fragen, auf die Tickets warten (abgeleitete Sperre).
export class BoardQuestionBlocking {
  // Offene Fragen je Ticket, sortiert nach Fragenummer; Tickets ohne offene Frage fehlen in der Map
  async openByTicket(u: BoardUnitOfWork, ticketIds: string[]): Promise<Map<string, BoardOpenQuestion[]>> {
    const result = new Map<string, BoardOpenQuestion[]>();
    if (!ticketIds.length) return result;
    const wanted = new Set(ticketIds);
    const questions = ticketIds.length > MANY_TICKETS
      ? await u.questions.list({ states: ['open'] })
      : await u.questions.list({ ticketIds, states: ['open'] });
    for (const question of questions) {
      for (const ticketId of question.ticketIds) {
        if (!wanted.has(ticketId)) continue;
        const list = result.get(ticketId) ?? [];
        list.push({ id: question.id, title: question.title });
        result.set(ticketId, list);
      }
    }
    return result;
  }
}
