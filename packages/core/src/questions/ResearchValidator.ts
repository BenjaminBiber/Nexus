import { HarnessError } from '../errors/HarnessError.js';
import type { Actor } from '../actors/Actor.js';
import type { QuestionOption, Research } from './Question.js';

export const MAX_OPTIONS = 5;

// Eingabe einer Recherche: Kontext und/oder Optionen (ohne Nummern, die vergibt das System)
export interface ResearchInput {
  context?: string;
  options?: Array<{ title?: string; text?: string; pros?: string[]; cons?: string[]; recommended?: boolean }>;
}

// Prüft eine Recherche und baut sie samt fortlaufend nummerierten Optionen (`O1`, `O2`, …).
export class ResearchValidator {
  // Liefert die Recherche; `existing` bestimmt, bei welcher Nummer die Optionen weiterzählen
  build(input: ResearchInput, existing: Research[], actor: Actor, at: string): Research {
    const context = (input.context ?? '').trim();
    const raw = input.options ?? [];
    if (!context && raw.length === 0) {
      throw new HarnessError('research-invalid', 'Eine Recherche braucht Kontext oder mindestens eine Option.');
    }
    if (raw.length > MAX_OPTIONS) {
      throw new HarnessError('research-invalid', `Eine Recherche hat höchstens ${MAX_OPTIONS} Optionen.`);
    }
    if (raw.filter((o) => o.recommended).length > 1) {
      throw new HarnessError('research-recommended-multiple', 'Höchstens eine Option je Recherche darf empfohlen sein.');
    }
    let next = this.highestNumber(existing);
    const options = raw.map((o): QuestionOption => {
      const title = (o.title ?? '').trim();
      const text = (o.text ?? '').trim();
      if (!title || !text) throw new HarnessError('research-invalid', 'Jede Option braucht einen Titel und einen Text.');
      next += 1;
      return { id: `O${next}`, title, text, pros: this.texts(o.pros), cons: this.texts(o.cons), recommended: o.recommended === true };
    });
    return { at, author: actor.author, kind: actor.kind, context, options };
  }

  // Text für den Verlauf, z. B. „3 Optionen (O1 bis O3), O2 empfohlen“; ohne Optionen „Kontext ergänzt“
  describe(research: Pick<Research, 'options'>): string {
    const options = research.options;
    if (options.length === 0) return 'Kontext ergänzt';
    const first = options[0].id;
    const last = options[options.length - 1].id;
    const count = options.length === 1 ? '1 Option' : `${options.length} Optionen`;
    const range = options.length === 1 ? `(${first})` : `(${first} bis ${last})`;
    const recommended = options.find((o) => o.recommended);
    return `${count} ${range}${recommended ? `, ${recommended.id} empfohlen` : ''}`;
  }

  // Höchste bisher vergebene Optionsnummer über alle Recherchen
  private highestNumber(existing: Research[]): number {
    const numbers = existing.flatMap((r) => r.options.map((o) => Number(o.id.slice(1))));
    return numbers.length ? Math.max(...numbers) : 0;
  }

  // Bereinigt eine Liste von Texten
  private texts(items: string[] | undefined): string[] {
    return (items ?? []).map((s) => String(s).trim()).filter(Boolean);
  }
}
