import type { BoardUnitOfWork } from './BoardDatabase.js';
import type { BoardActor, Ticket } from './BoardTypes.js';

// Entfernt beim Löschen eines Tickets dessen Bezüge an Fragen (betroffen und Abhängigkeit) und schreibt den Verlauf der Fragen.
export class QuestionTicketCleanup {
  // Läuft vor dem Löschen des Tickets; jede betroffene Frage bekommt eine neue Version und einen Eintrag „ticket-deleted“.
  // Sperrreihenfolge wie beim Setzen der Tickets einer Frage: erst die Fragezeilen (sortiert), dann die Bezüge.
  async remove(u: BoardUnitOfWork, ticket: Pick<Ticket, 'id' | 'title'>, actor: BoardActor, at: string): Promise<string[]> {
    const referencing = [...(await u.questions.idsReferencingTicket(ticket.id))].sort();
    for (const questionId of referencing) await u.questions.byId(questionId, true);
    const affected = await u.questions.removeTicket(ticket.id);
    for (const questionId of affected) {
      // Frisch gelesen (ohne den entfernten Bezug), damit update ihn nicht zurückschreibt
      const current = (await u.questions.byId(questionId, true))!;
      await u.questions.update({ ...current, rev: current.rev + 1, updatedAt: at });
      await u.questionHistory.insert([{
        questionId, at, byId: actor.userId, byName: actor.name, byKind: null, type: 'ticket-deleted',
        text: `Ticket ${ticket.id} „${ticket.title}“ gelöscht, Bezug entfernt.`,
      }]);
    }
    return affected;
  }
}
