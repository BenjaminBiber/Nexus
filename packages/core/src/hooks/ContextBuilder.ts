import { HarnessError } from '../errors/HarnessError.js';
import type { BoardServices } from '../board/BoardServices.js';
import type { Comment } from '../board/BoardTypes.js';
import type { BoardQuestion } from '../board/BoardQuestionTypes.js';
import type { TicketDetail } from '../board/TicketService.js';
import { WorkflowHint } from '../workflow/WorkflowHint.js';
import { WORKFLOW_PHASES, type WorkflowPhase } from '../workflow/WorkflowPhases.js';
import type { QuestionContextMarks } from './QuestionContextMarks.js';

const MAX_QUESTIONS = 5;

/** Ergebnis von ContextBuilder.build: einzuspeisender Text und die Kommentare darin, die nach der Ausgabe als gelesen gelten */
export interface HookContext {
  text: string | null;
  commentIds: number[];
}

// Baut den Text, den ein Hook in die Session einspeist: Leitfaden zum Sessionbeginn, neue Kommentare und offene Fragen.
// Alles kommt aus der Board-DB; eingespeiste Kommentare markiert der Aufrufer erst nach der Ausgabe mit `markRead`.
export class ContextBuilder {
  private readonly workflow: WorkflowHint;

  // `phases`: angewendeter Workflow (WorkflowSource), ohne Angabe der Standard
  constructor(
    private readonly services: Pick<BoardServices, 'tickets' | 'comments' | 'db'>,
    private readonly marks: QuestionContextMarks | null = null,
    private readonly now: () => Date = () => new Date(),
    phases: WorkflowPhase[] = WORKFLOW_PHASES,
  ) {
    this.workflow = new WorkflowHint(phases);
  }

  // `session-start`: Leitfaden, Kommentare und offene Fragen; `user-prompt`: Kommentare und neue oder geänderte Fragen.
  // Mit `sessionId` merkt sich der Harness die Einspeisung der Fragen. Text null, wenn es nichts gibt.
  // DB-Fehler (außer fehlenden Tickets) werfen; der Aufrufer speist dann keinen Kontext ein.
  async build(kind: 'session-start' | 'user-prompt', ticketIds: string[], sessionId?: string): Promise<HookContext> {
    const tickets = await this.load(ticketIds);
    const parts: string[] = [];
    if (kind === 'session-start') parts.push(this.guide(tickets));
    const comments = await this.comments(tickets);
    parts.push(...comments.parts);
    const questions = await this.questions(kind, tickets, sessionId);
    if (questions) parts.push(questions);
    return { text: parts.length ? parts.join('\n\n') : null, commentIds: comments.ids };
  }

  // Markiert eingespeiste Kommentare als gelesen; erst aufrufen, wenn der Kontext ausgegeben ist
  async markRead(commentIds: number[]): Promise<void> {
    await this.services.comments.markReadByAgent(commentIds);
  }

  // Ungelesene Kommentare je Ticket und ihre IDs; als gelesen markiert erst `markRead`
  private async comments(tickets: TicketDetail[]): Promise<{ parts: string[]; ids: number[] }> {
    if (!tickets.length) return { parts: [], ids: [] };
    const unread = await this.services.comments.unreadForAgent(tickets.map((t) => t.id));
    const parts = tickets.flatMap((t) => {
      const own = unread.filter((c) => c.ticketId === t.id);
      return own.length ? [this.commentList(t.id, own)] : [];
    });
    return { parts, ids: unread.map((c) => c.id) };
  }

  // Offene Fragen zu den Tickets; beim Prompt mit Session nur neue oder geänderte seit der letzten Einspeisung
  private async questions(kind: 'session-start' | 'user-prompt', tickets: TicketDetail[], sessionId?: string): Promise<string | null> {
    if (!tickets.length) return null;
    const ids = tickets.map((t) => t.id);
    const since = kind === 'user-prompt' && sessionId && this.marks ? await this.marks.get(sessionId) : null;
    // Zeitpunkt vor dem Lesen festhalten, damit eine Änderung während des Lesens beim nächsten Mal nicht verloren geht
    const readAt = this.now().toISOString();
    const open = await this.services.db.transaction((u) => u.questions.list({ ticketIds: ids, states: ['open'] }));
    const news = open.filter((q) => since === null || q.updatedAt > since);
    if (sessionId && this.marks && (kind === 'session-start' || news.length)) await this.marks.set(sessionId, readAt);
    return news.length ? this.questionList(news, ids) : null;
  }

  // Liste der offenen Fragen mit Regeln für den Agenten
  private questionList(questions: BoardQuestion[], ticketIds: string[]): string {
    const shown = questions.slice(0, MAX_QUESTIONS);
    const count = questions.length > MAX_QUESTIONS ? `${MAX_QUESTIONS} von ${questions.length}` : `${questions.length}`;
    const lines = shown.map((q) => `- ${q.id} ${q.title} (betrifft ${q.ticketIds.filter((t) => ticketIds.includes(t)).join(', ')})`);
    const first = shown[0].id;
    return [
      `Offene Fragen (${count}, nicht entscheiden):`,
      ...lines,
      'Diese Fragen sperren ihre Tickets für In Arbeit und Erledigt. Triff die Entscheidungen nie selbst und nimm nichts stillschweigend an.',
      `Ergänze Kontext mit \`harness board question research ${first} --from <datei> --kind session\` oder kommentiere mit \`harness board question comment ${first} "…" --kind session\`.`,
    ].join('\n');
  }

  // Lädt die Tickets; unbekannte IDs werden übergangen, andere Fehler (z. B. DB weg) weitergereicht
  private async load(ids: string[]): Promise<TicketDetail[]> {
    const tickets: TicketDetail[] = [];
    for (const id of ids) {
      try {
        tickets.push(await this.services.tickets.get(id));
      } catch (error) {
        if (!(error instanceof HarnessError && error.code.endsWith('-not-found'))) throw error; // Ticket gibt es nicht (mehr)
      }
    }
    return tickets;
  }

  // Leitfaden zum Sessionbeginn
  private guide(tickets: TicketDetail[]): string {
    const hint = this.workflow.build(tickets);
    if (tickets.length === 0) {
      return [
        'AI Harness: Diese Session ist keinem Ticket zugeordnet. Mit `harness board claim ARC-0042 --agent claude` ordnest du sie einem Ticket zu (ID anpassen).',
        hint,
      ].join('\n');
    }
    const names = tickets.map((t) => `${t.id} „${t.title}“${this.meta(t)}`).join(', ');
    const first = tickets[0].id;
    return [
      `AI Harness: Diese Session arbeitet an ${names}.`,
      `Regeln: Tickets änderst du nur über \`harness board …\`. Nutze \`harness board show ${first}\`, \`harness board comment ${first} "…" --author claude\`, \`harness board step ${first} <nummer>\` und zum Abschluss \`harness board done ${first} --acceptance "…"\`. Begründe jede Änderung am Ticket mit einem Kommentar.`,
      hint,
    ].join('\n');
  }

  // Spec und Fortschritt der Schritte in Klammern, sofern vorhanden
  private meta(t: TicketDetail): string {
    const parts = [t.spec ? `Spec ${t.spec}` : '', t.stepsTotal ? `Schritte ${t.stepsDone}/${t.stepsTotal}` : ''].filter(Boolean);
    return parts.length ? ` (${parts.join(', ')})` : '';
  }

  // Neue Kommentare eines Tickets, in zeitlicher Reihenfolge
  private commentList(id: string, comments: Comment[]): string {
    const lines = comments.map((c) => `- ${this.date(c.createdAt)}, ${c.authorName}: ${c.body.replace(/\n/g, '\n  ')}`);
    return [`Neue Kommentare zu ${id}:`, ...lines].join('\n');
  }

  // Datum und Uhrzeit, deutsch
  private date(iso: string): string {
    return new Date(iso).toLocaleString('de-DE', { day: '2-digit', month: '2-digit', year: 'numeric', hour: '2-digit', minute: '2-digit' });
  }
}
