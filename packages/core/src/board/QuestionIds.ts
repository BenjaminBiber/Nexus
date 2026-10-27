import { HarnessError } from '../errors/HarnessError.js';
import { TicketIds } from './TicketIds.js';

const QUESTION_ID = /^([A-Za-z][A-Za-z0-9]{1,5})-[Ff](\d+)$/;

// Fragenummern der Form „ARC-F0001“ bilden und erkennen; Verweise auf Fragen oder Tickets vereinheitlichen.
export class QuestionIds {
  private readonly tickets = new TicketIds();

  // Bildet die Nummer mit mindestens vier Ziffern
  format(key: string, n: number): string {
    return `${key}-F${String(n).padStart(4, '0')}`;
  }

  // Ist der Text eine Fragenummer (Schreibweise egal)?
  isQuestion(id: string): boolean {
    return QUESTION_ID.test(id.trim());
  }

  // Schreibweise vereinheitlichen (Großbuchstaben, vier Ziffern)
  canonical(id: string): string {
    const match = QUESTION_ID.exec(id.trim());
    if (!match) throw new HarnessError('question-id-invalid', `„${id}“ ist keine Fragenummer (erwartet z. B. ARC-F0001).`);
    return this.format(match[1].toUpperCase(), Number(match[2]));
  }

  // Projektkey einer Fragenummer
  keyOf(id: string): string {
    return this.canonical(id).split('-')[0];
  }

  // Verweis auf eine Frage oder ein Ticket in einheitlicher Schreibweise
  reference(id: string): string {
    if (this.isQuestion(id)) return this.canonical(id);
    try {
      return this.tickets.canonical(id);
    } catch {
      throw new HarnessError('reference-invalid', `„${id}“ ist weder eine Frage (z. B. ARC-F0001) noch ein Ticket (z. B. ARC-0001).`);
    }
  }
}
