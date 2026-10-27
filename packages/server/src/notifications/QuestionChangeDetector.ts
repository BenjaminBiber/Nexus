import type { AuthorKind, QuestionOption } from '@harness/core';

// Was eine Frage für Meldungen braucht: eine Datei-Frage oder eine Frage der Board-Datenbank
export interface NotifiableQuestion {
  id: string;
  title: string;
  occasion: string | null;
  origin: { kind: AuthorKind };
  research: Array<{ kind: AuthorKind; options: QuestionOption[] }>;
}

export interface FoundQuestionChange<Q extends NotifiableQuestion> {
  kind: 'frage-neu' | 'recherche';
  question: Q;
}

// Erkennt zwischen zwei Ständen neue Fragen und neue Recherchen von Agenten; merkt sich je Frage die Zahl der Recherchen,
// damit jede Änderung genau einmal gemeldet wird.
export class QuestionChangeDetector<Q extends NotifiableQuestion = NotifiableQuestion> {
  private readonly known = new Map<string, number>();
  private primedOnce = false;

  // Wurde der Bestand schon einmal eingelesen?
  get primed(): boolean {
    return this.primedOnce;
  }

  // Merkt sich den Bestand, ohne etwas zu melden
  prime(questions: Q[]): void {
    questions.forEach((q) => this.known.set(q.id, q.research.length));
    this.primedOnce = true;
  }

  // Vergleicht mit dem gemerkten Stand; vor dem ersten Einlesen wird nur eingelesen (nichts gemeldet)
  compare(questions: Q[]): Array<FoundQuestionChange<Q>> {
    if (!this.primedOnce) {
      this.prime(questions);
      return [];
    }
    const found: Array<FoundQuestionChange<Q>> = [];
    for (const question of questions) {
      const before = this.known.get(question.id);
      this.known.set(question.id, question.research.length);
      const kind = this.kindOf(question, before);
      if (kind) found.push({ kind, question });
    }
    return found;
  }

  // Neue Frage eines Agenten oder neue Recherche eines Agenten; sonst nichts
  private kindOf(question: Q, before: number | undefined): 'frage-neu' | 'recherche' | null {
    if (before === undefined) return question.origin.kind === 'human' ? null : 'frage-neu';
    if (question.research.length <= before) return null;
    const latest = question.research[question.research.length - 1];
    return latest.kind === 'human' ? null : 'recherche';
  }
}
