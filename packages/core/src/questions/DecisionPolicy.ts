import { HarnessError } from '../errors/HarnessError.js';
import type { Actor } from '../actors/Actor.js';
import type { Decision, Question } from './Question.js';

// Eingabe einer Entscheidung: genau eines von Option und Freitext, dazu wahlweise eine Begründung
export interface DecisionInput {
  option?: string | null;
  text?: string | null;
  reason?: string;
}

// Was die Regeln von einer Frage brauchen: eine Datei-Frage oder eine Frage der Board-Datenbank
// (deren Recherchen im Datei-Format übergeben werden)
export type DecisionSubject = Pick<Question, 'id' | 'state' | 'research'>;

// Regeln für Entscheiden, Verwerfen und Wiedereröffnen: nur Menschen, nur offene Fragen; Verwerfen nur mit Begründung.
export class DecisionPolicy {
  // Nur ein Mensch darf entscheiden, verwerfen oder wieder öffnen
  assertHuman(actor: Actor): void {
    if (actor.kind !== 'human') {
      throw new HarnessError(
        'human-only',
        'Diese Entscheidung trifft ein Mensch. Bitte in der Oberfläche oder in einem eigenen Terminal ausführen.',
      );
    }
  }

  // Prüft eine Entscheidung und liefert sie; die Frage muss offen sein
  answer(question: DecisionSubject, input: DecisionInput, actor: Actor, at: string): Decision {
    this.assertHuman(actor);
    this.assertState(question, 'open');
    const option = input.option?.trim() || null;
    const text = input.text?.trim() || null;
    if ((option === null) === (text === null)) {
      throw new HarnessError('decision-ambiguous', 'Bitte entweder eine Option wählen oder einen eigenen Text eingeben, nicht beides und nicht keines.');
    }
    // Beim Entscheiden ist die Begründung freiwillig (nur Verwerfen verlangt eine)
    const reason = (input.reason ?? '').trim();
    if (option !== null) this.assertOption(question, option);
    return { outcome: 'answered', option, text, reason, by: actor.author, at };
  }

  // Prüft das Verwerfen und liefert die Entscheidung; die Begründung ist Pflicht
  discard(question: Pick<Question, 'id' | 'state'>, reason: string | undefined, actor: Actor, at: string): Decision {
    this.assertHuman(actor);
    this.assertState(question, 'open');
    return { outcome: 'discarded', option: null, text: null, reason: this.reason(reason), by: actor.author, at };
  }

  // Prüft, dass die Frage wieder geöffnet werden darf (entschieden oder verworfen)
  assertReopenable(question: Pick<Question, 'id' | 'state'>, actor: Actor): void {
    this.assertHuman(actor);
    if (question.state === 'open') {
      throw new HarnessError('question-state-invalid', `Die Frage ${question.id} ist bereits offen.`);
    }
  }

  // Beschreibt eine Entscheidung für den Verlauf, z. B. „Option O2 „ExcelJS“ gewählt. Begründung: …“
  describe(question: Pick<Question, 'research'>, decision: Decision): string {
    if (decision.outcome === 'discarded') return `Verworfen. Begründung: ${decision.reason}`;
    const because = decision.reason ? ` Begründung: ${decision.reason}` : '';
    if (decision.option) {
      const title = this.optionTitle(question, decision.option);
      return `Option ${decision.option}${title ? ` „${title}“` : ''} gewählt.${because}`;
    }
    return `Antwort: ${decision.text}.${because}`;
  }

  // Titel einer Option aus den Recherchen (neueste zuerst)
  private optionTitle(question: Pick<Question, 'research'>, id: string): string | null {
    for (const research of [...question.research].reverse()) {
      const found = research.options.find((o) => o.id === id);
      if (found) return found.title;
    }
    return null;
  }

  // Die Frage muss im erwarteten Zustand sein
  private assertState(question: Pick<Question, 'id' | 'state'>, expected: 'open'): void {
    if (question.state !== expected) {
      throw new HarnessError('question-state-invalid', `Die Frage ${question.id} ist nicht offen. Zuerst wieder öffnen.`);
    }
  }

  // Gewählt werden darf nur eine Option der neuesten Recherche
  private assertOption(question: Pick<Question, 'research'>, id: string): void {
    const newest = question.research.at(-1);
    if (!newest || !newest.options.some((o) => o.id === id)) {
      throw new HarnessError('decision-option-unknown', `Die Option ${id} gibt es in der neuesten Recherche nicht.`);
    }
  }

  // Begründung, die Pflicht ist (Verwerfen)
  private reason(value: string | undefined): string {
    const reason = (value ?? '').trim();
    if (!reason) throw new HarnessError('decision-reason-missing', 'Bitte eine Begründung angeben.');
    return reason;
  }
}
