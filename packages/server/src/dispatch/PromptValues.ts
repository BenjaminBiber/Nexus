import {
  ReviewFeedback,
  type AnsweredQuestion, type BoardComment, type BoardTicketDetail, type DispatchConfig,
  type MergePromptInput, type PlanPromptInput, type ResearchPromptInput, type WorkPromptInput,
} from '@harness/core';
import type { DispatchContext } from './DispatchContext.js';
import { PlanRunner } from './PlanRunner.js';
import { ResearchRunner } from './ResearchRunner.js';

/** Zielbranch, wenn keiner bekannt ist – die Vorschau fasst kein Repo an */
const DEFAULT_TARGET = 'main';

// Die Werte, mit denen die Arbeitsanweisungen gefüllt werden. Lauf und Vorschau holen sie hier,
// damit die Vorschau genau das zeigt, was der Agent später bekommt.
export class PromptValues {
  private readonly feedback = new ReviewFeedback();

  constructor(private readonly ctx: DispatchContext) {}

  // Entschiedene Fragen zum Ticket für die Arbeitsanweisung
  async answers(ticketId: string): Promise<AnsweredQuestion[]> {
    const key = ticketId.slice(0, ticketId.lastIndexOf('-'));
    const { questions } = await this.ctx.services.questions.list(key, { ticket: ticketId, state: 'answered' });
    return questions.flatMap((q) => {
      if (!q.decision) return [];
      const what = [q.decision.option, q.decision.text].filter(Boolean).join(' – ') || 'entschieden';
      return [{ id: q.id, title: q.title, decision: `${what} (Begründung: ${q.decision.reason})` }];
    });
  }

  // Offene Rückmeldungen aus dem Review seit dem letzten Start
  pending(comments: Pick<BoardComment, 'authorName' | 'body'>[]): string[] {
    return this.feedback.pending(comments, this.ctx.cfg.dispatcherName);
  }

  // Werte eines Umsetzungslaufs zu einem echten Ticket
  async work(detail: BoardTicketDetail): Promise<WorkPromptInput> {
    return {
      ticketId: detail.id,
      title: detail.title,
      agent: this.ctx.agentName(detail.id),
      branch: detail.branch ?? this.sampleBranch(detail.id),
      answers: await this.answers(detail.id),
      feedback: this.pending(detail.comments),
      cfg: this.ctx.cfg,
    };
  }

  // Werte eines Planungslaufs zu einem echten Ticket
  plan(detail: BoardTicketDetail): PlanPromptInput {
    return { ticketId: detail.id, title: detail.title, agent: PlanRunner.agentName(detail.id), cfg: this.ctx.cfg };
  }

  // Werte eines Merge-Laufs zu einem echten Ticket; der Zielbranch bleibt in der Vorschau der Standard
  merge(detail: BoardTicketDetail): MergePromptInput {
    return {
      ticketId: detail.id,
      title: detail.title,
      agent: this.ctx.agentName(detail.id),
      branch: detail.branch ?? this.sampleBranch(detail.id),
      target: DEFAULT_TARGET,
      cfg: this.ctx.cfg,
    };
  }

  // Werte eines Recherche-Laufs zu einer echten Frage
  research(question: { id: string; title: string }): ResearchPromptInput {
    return { questionId: question.id, title: question.title, agent: ResearchRunner.agentName(question.id), cfg: this.ctx.cfg };
  }

  private sampleBranch(ticketId: string): string {
    return `ticket/${ticketId.toLowerCase()}`;
  }

  // Beispielwerte ohne Board: eine Antwort und eine Review-Rückmeldung sind dabei, damit die
  // optionalen Blöcke der Vorlage in der Vorschau sichtbar werden.
  static sample(cfg: DispatchConfig): {
    work: WorkPromptInput; plan: PlanPromptInput; research: ResearchPromptInput; merge: MergePromptInput;
  } {
    const ticketId = 'AIH-0042';
    const title = 'Beispielticket: Vorschau der Arbeitsanweisung';
    const agent = `${cfg.agentPrefix}${ticketId}`;
    const branch = 'ticket/AIH-0042-beispiel';
    return {
      work: {
        ticketId, title, agent, branch, cfg,
        answers: [{ id: 'AIH-F0007', title: 'Beispielfrage: Welche Variante?', decision: 'Variante B (Begründung: weniger Aufwand)' }],
        feedback: ['Beispiel-Rückmeldung aus dem Review: Bitte noch einen Test für den Fehlerfall.'],
      },
      plan: { ticketId, title, agent: PlanRunner.agentName(ticketId), cfg },
      research: { questionId: 'AIH-F0007', title: 'Beispielfrage: Welche Variante?', agent: ResearchRunner.agentName('AIH-F0007'), cfg },
      merge: { ticketId, title, agent, branch, target: DEFAULT_TARGET, cfg },
    };
  }
}
