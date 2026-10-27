// Was zur Frage bekannt sein muss: wer sie gestellt hat und auf welchen Boards ihre Tickets liegen
export interface ConcernSubject {
  createdById: string;
  boardIds: number[];
}

// Betrifft eine Frage den Benutzer („Eigene“)? Selbst gestellt (auch von einem Agent unter dem eigenen Benutzer) oder mit einem
// Ticket auf einem eigenen Board verknüpft. Gemeinsame Regel für Fragenliste, Zähler, Meldungen und Recherche-Automatik.
export class QuestionConcern {
  concerns(question: ConcernSubject, userId: string, ownBoardIds: ReadonlySet<number>): boolean {
    return question.createdById === userId || question.boardIds.some((id) => ownBoardIds.has(id));
  }
}
