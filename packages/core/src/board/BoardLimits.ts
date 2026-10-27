import { HarnessError } from '../errors/HarnessError.js';
import { MAX_PERSON_NAME_LENGTH, type BoardActor } from './BoardTypes.js';

// Prüft Texte gegen die Höchstlängen der Datenbank-Spalten, bevor SQL sie abschneidet oder ablehnt.
export class BoardLimits {
  // Liefert den Text unverändert oder wirft „value-too-long“; null bleibt null
  static text<T extends string | null>(value: T, max: number, what: string): T {
    if (value !== null && value.length > max) {
      throw new HarnessError('value-too-long', `${what} ist zu lang (${value.length} Zeichen, höchstens ${max}).`);
    }
    return value;
  }

  // Name des Handelnden (Verlauf, Kommentare, Notizen, Löschprotokoll) – die eine Stelle für alle Dienste
  static actor(actor: BoardActor): BoardActor {
    BoardLimits.text(actor.name, MAX_PERSON_NAME_LENGTH, 'Der Name des Handelnden');
    return actor;
  }
}
