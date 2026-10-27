import type { DispatchNotice } from '../dispatch/DispatchContext.js';
import type { QuestionOption, SessionView } from '@harness/core';
import type { NotifiableQuestion } from './QuestionChangeDetector.js';
import type { PendingSession } from './AttentionTracker.js';
import type { ToastModel } from './ToastModel.js';

export interface SingleInput {
  kind: 'bereit' | 'wartet';
  projectName: string;
  ticket?: { id: string; title: string };
  session: Pick<SessionView, 'title' | 'model'>;
  openUrl: string;
}

export interface QuestionNewInput {
  projectName: string;
  question: NotifiableQuestion;
  openUrl: string;
}

export interface ResearchTextInput extends QuestionNewInput {
  research: { options: QuestionOption[] };
}

// Formuliert die Texte der Toasts (siehe docs/design/toast/toast-spec.md)
export class ToastTexts {
  // Einzelmeldung: Titel, `Projekt · Ticket`, `Sessiontitel · Modell`
  single(input: SingleInput): ToastModel {
    const { kind, projectName, ticket, session } = input;
    const line1 = ticket ? `${projectName} · ${ticket.id} ${ticket.title}`.trim() : projectName;
    const line2 = [session.title, session.model].filter(Boolean).join(' · ') || 'Zurück zur Session';
    const title = kind === 'bereit' ? 'Session ist fertig' : 'Session braucht deine Antwort';
    return { kind, title, line1, line2, openUrl: input.openUrl };
  }

  // Neue Frage eines Agenten: `Projekt · F-Nr Titel`, `Anlass: … · noch keine Recherche` (oder „Recherche liegt vor“)
  questionNew(input: QuestionNewInput): ToastModel {
    const { question } = input;
    const occasion = question.occasion ?? 'nicht angegeben';
    return {
      kind: 'frage-neu',
      title: 'Neue Frage von einer Session',
      line1: this.questionLine(input.projectName, question),
      line2: this.shorten(`Anlass: ${occasion} · ${question.research.length ? 'Recherche liegt vor' : 'noch keine Recherche'}`),
      openUrl: input.openUrl,
    };
  }

  // Neue Recherche eines Agenten: Zahl und Bereich der Optionen, Empfehlung; ohne Optionen „Kontext ergänzt“
  research(input: ResearchTextInput): ToastModel {
    return {
      kind: 'recherche',
      title: 'Recherche ist da',
      line1: this.questionLine(input.projectName, input.question),
      line2: this.optionsLine(input.research.options),
      openUrl: input.openUrl,
    };
  }

  // Meldung des Dispatchers: Titel je Art, `ID Titel`, Text (gemergt, blockiert, Frage, Pause, Review)
  dispatch(input: { notice: DispatchNotice; openUrl: string }): ToastModel {
    const { notice } = input;
    const titles = { merged: 'Dispatcher: gemergt', blocked: 'Dispatcher: blockiert', question: 'Dispatcher: Frage vom Agent', paused: 'Dispatcher pausiert', review: 'Dispatcher: wartet auf Review' };
    return {
      kind: notice.kind === 'merged' ? 'bereit' : 'wartet',
      title: titles[notice.kind],
      line1: this.shorten(`${notice.ticketId ?? ''} ${notice.title}`.trim()),
      line2: this.shorten(notice.text),
      openUrl: input.openUrl,
    };
  }

  // `Projekt · F-Nr Titel`, gekürzt
  private questionLine(projectName: string, question: Pick<NotifiableQuestion, 'id' | 'title'>): string {
    return this.shorten(`${projectName} · ${question.id} ${question.title}`.trim());
  }

  // `3 Optionen (O3 bis O5), O4 empfohlen`
  private optionsLine(options: QuestionOption[]): string {
    if (options.length === 0) return 'Kontext ergänzt';
    const count = options.length === 1 ? '1 Option' : `${options.length} Optionen`;
    const range = options.length === 1 ? options[0].id : `${options[0].id} bis ${options[options.length - 1].id}`;
    const recommended = options.find((o) => o.recommended);
    return `${count} (${range})${recommended ? `, ${recommended.id} empfohlen` : ''}`;
  }

  // Kürzt lange Zeilen mit „…“
  private shorten(line: string, max = 80): string {
    return line.length > max ? `${line.slice(0, max - 1)}…` : line;
  }

  // Sammelmeldung für zwei oder mehr wartende Sessions
  collected(pending: PendingSession[], boardUrl: string): ToastModel {
    const waiting = pending.filter((p) => p.kind === 'wartet').length;
    const ready = pending.length - waiting;
    const parts = [
      waiting > 0 ? `${waiting} ${waiting === 1 ? 'braucht' : 'brauchen'} eine Antwort` : '',
      ready > 0 ? `${ready} ${ready === 1 ? 'ist' : 'sind'} fertig` : '',
    ].filter(Boolean);
    const shown = pending.slice(0, 3).map((p) => p.label);
    const rest = pending.length - shown.length;
    return {
      kind: 'gesammelt',
      title: `${pending.length} Sessions warten auf dich`,
      line1: parts.join(', '),
      line2: rest > 0 ? `${shown.join(' · ')} + ${rest} weitere` : shown.join(' · '),
      openUrl: boardUrl,
    };
  }
}
