import { BoardLimits } from '../board/BoardLimits.js';
import { MAX_BRANCH_LENGTH } from '../board/BoardTypes.js';
import { HarnessError } from '../errors/HarnessError.js';

// Leerzeichen, Steuerzeichen und die Zeichen ~ ^ : ? * [ \ sind in Branch-Namen verboten (git check-ref-format)
const FORBIDDEN_CHARACTERS = /[\u0000- \u007f~^:?*[\\]/;
// Der Namensraum der Ticket-Branches, unabhängig von Groß-/Kleinschreibung
const TICKET_NAMESPACE = /^ticket(\/|$)/i;

// Regeln für den Namen eines Zielbranches, nachgebildet nach „git check-ref-format --branch“, aber ohne Git-Aufruf.
export class TargetBranchRule {
  // Liefert den getrimmten Namen, null bei leerem Wert; wirft „target-branch-invalid“ oder „value-too-long“
  normalize(value: string | null | undefined): string | null {
    const name = value?.trim() ?? '';
    if (name === '') return null;
    BoardLimits.text(name, MAX_BRANCH_LENGTH, 'Der Zielbranch');
    if (!this.isValidName(name)) {
      throw new HarnessError('target-branch-invalid', `„${name}“ ist kein gültiger Branch-Name.`);
    }
    // „ticket“ kollidiert mit refs/heads/ticket/*, „Ticket/…“ trifft unter Windows die Branches der Tickets
    if (TICKET_NAMESPACE.test(name)) {
      throw new HarnessError('target-branch-invalid', `„${name}“ ist der Branch eines Tickets; dorthin wird nicht gemergt.`);
    }
    return name;
  }

  // Erfüllt der Name die Namensregeln von Git?
  private isValidName(name: string): boolean {
    if (name.startsWith('-') || name === '@' || name === 'HEAD') return false;
    if (FORBIDDEN_CHARACTERS.test(name) || name.includes('..') || name.includes('@{')) return false;
    if (name.endsWith('.')) return false;
    // Jeder Abschnitt zwischen den Schrägstrichen: nicht leer (deckt „//“ und Schrägstrich am Anfang/Ende ab), kein Punkt am Anfang, nicht „.lock“ am Ende
    return name.split('/').every((section) => section !== '' && !section.startsWith('.') && !section.endsWith('.lock'));
  }
}
