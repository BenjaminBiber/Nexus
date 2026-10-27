import type { BoardUnitOfWork } from './BoardDatabase.js';
import type { BoardQuestion, BoardQuestionDetail, BoardQuestionView, BoardQuestionWaiting } from './BoardQuestionTypes.js';
import type { Ticket } from './BoardTypes.js';
import { QuestionIds } from './QuestionIds.js';

// Berechnet die Anzeigefassung von Fragen: betroffene Tickets mit Titel, offene Abhängigkeiten, bereit, recherchiert.
export class BoardQuestionViews {
  private readonly ids = new QuestionIds();

  // Anzeigefassungen in einer Transaktion; Ziele der Abhängigkeiten werden projektübergreifend aufgelöst
  async build(u: BoardUnitOfWork, questions: BoardQuestion[]): Promise<BoardQuestionView[]> {
    if (!questions.length) return [];
    const ticketRefs = [...new Set(questions.flatMap((q) => [...q.ticketIds, ...q.dependsOn.filter((d) => !this.ids.isQuestion(d))]))];
    const questionRefs = [...new Set(questions.flatMap((q) => q.dependsOn.filter((d) => this.ids.isQuestion(d))))];
    const tickets = new Map((ticketRefs.length ? await u.tickets.list({ ids: ticketRefs }) : []).map((t) => [t.id, t]));
    const others = new Map((questionRefs.length ? await u.questions.list({ ids: questionRefs }) : []).map((q) => [q.id, q]));
    const keys = new Map((await u.projects.list()).map((p) => [p.id, p.key]));
    const research = await u.questionResearch.summaries(questions.map((q) => q.id));
    return questions.map((q) => {
      const waitingOn = this.waitingOn(q, tickets, others);
      const summary = research[q.id];
      return {
        ...q,
        projectKey: keys.get(q.projectId) ?? '?',
        tickets: q.ticketIds.flatMap((id) => {
          const t = tickets.get(id);
          return t ? [{ id, title: t.title, status: t.status, boardId: t.boardId }] : [];
        }),
        waitingOn,
        ready: q.state === 'open' && waitingOn.length === 0,
        researched: (summary?.count ?? 0) > 0,
        optionCount: summary?.optionCount ?? 0,
      };
    });
  }

  // Anzeigefassung samt Recherchen, Kommentaren und Verlauf
  async detail(u: BoardUnitOfWork, question: BoardQuestion): Promise<BoardQuestionDetail> {
    const [view] = await this.build(u, [question]);
    return {
      ...view,
      research: await u.questionResearch.list(question.id),
      comments: await u.questionComments.list(question.id),
      history: await u.questionHistory.list(question.id),
    };
  }

  // Offene Abhängigkeiten: Frage noch offen, Ticket weder erledigt noch „nicht mehr nötig“; unbekannte Ziele zählen nicht
  private waitingOn(q: BoardQuestion, tickets: Map<string, Ticket>, others: Map<string, BoardQuestion>): BoardQuestionWaiting[] {
    const open: BoardQuestionWaiting[] = [];
    for (const id of q.dependsOn) {
      const ticket = tickets.get(id);
      const other = others.get(id);
      if (ticket && ticket.status !== 'done' && ticket.status !== 'not_needed') open.push({ id, state: ticket.status });
      if (other && other.state === 'open') open.push({ id, state: other.state });
    }
    return open;
  }
}
