import type { BoardDatabase } from './BoardDatabase.js';
import type { BoardQuestionHistoryType } from './BoardQuestionTypes.js';
import { TicketIds } from './TicketIds.js';
import { HarnessError } from '../errors/HarnessError.js';
import { sortTimeline, type TicketTimeline, type TimelineAgentNames, type TimelineEvent } from './TicketTimeline.js';
import { TimelineClassifier } from './TimelineClassifier.js';

// Einträge im Verlauf einer Frage, die für die Zeitleiste eines betroffenen Tickets zählen
const QUESTION_TYPES: ReadonlySet<BoardQuestionHistoryType> = new Set(['created', 'research', 'tickets', 'decided', 'discarded', 'reopened']);

// Zeitleiste eines Tickets aus der Board-Datenbank: Verlauf, Kommentare (samt Dispatcher-Ereignissen) und verknüpfte Fragen.
// Lokale Quellen (Lauf-Protokolle, Git) ergänzt der Server.
export class TicketTimelineService {
  private readonly classifier: TimelineClassifier;
  private readonly ids = new TicketIds();

  constructor(private readonly db: BoardDatabase, names: TimelineAgentNames | null) {
    this.classifier = new TimelineClassifier(names);
  }

  // Liest alles in einer Transaktion; unbekannte ID ergibt ticket-not-found
  get(id: string): Promise<TicketTimeline> {
    const ticketId = this.ids.canonical(id);
    return this.db.transaction(async (u) => {
      if (!(await u.tickets.byId(ticketId))) throw new HarnessError('ticket-not-found', `Ticket ${ticketId} gibt es nicht.`);
      const history = await u.history.list(ticketId);
      const comments = await u.comments.list(ticketId);
      const questions = await u.questions.list({ ticketIds: [ticketId] });
      const linked = new Set(questions.map((q) => q.id));
      const events: TimelineEvent[] = [];
      for (const h of history) {
        const event = this.classifier.history(h);
        // Zustandswechsel verknüpfter Fragen kommen aus deren eigenem Verlauf (mit Art des Autors)
        if (h.field === 'question' && event.ref?.questionId && linked.has(event.ref.questionId)) continue;
        events.push(event);
      }
      for (const q of questions) {
        for (const h of await u.questionHistory.list(q.id)) {
          if (!QUESTION_TYPES.has(h.type)) continue;
          events.push({
            at: h.at, kind: 'question', type: `question:${h.type}`, actor: this.classifier.actor(h.byName, h.byKind),
            text: `${q.id} „${q.title}“: ${h.text}`, ref: { questionId: q.id },
          });
        }
      }
      events.push(...comments.map((c) => this.classifier.comment(c)));
      return { ticketId, events: sortTimeline(events) };
    });
  }
}
