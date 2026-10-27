import { HarnessError } from '../errors/HarnessError.js';
import { TICKET_STATUSES, type TicketStatus } from './BoardTypes.js';

// Deutsche und englische Schreibweisen der Spalten, wie im alten Board
const ALIASES: Record<string, TicketStatus> = {
  backlog: 'backlog',
  'in-planung': 'in_planning', planung: 'in_planning', verfeinern: 'in_planning',
  geplant: 'planned', plan: 'planned',
  'in-arbeit': 'in_progress', wip: 'in_progress', arbeit: 'in_progress',
  blockiert: 'blocked',
  erledigt: 'done', fertig: 'done',
  'nicht-mehr-noetig': 'not_needed', 'nicht-mehr-nötig': 'not_needed', 'nicht-noetig': 'not_needed',
  unnoetig: 'not_needed', hinfaellig: 'not_needed',
};

// Übersetzt eine Spaltenangabe aus CLI oder Oberfläche in einen Status.
export class StatusParser {
  // Liefert den Status; „wartet“ und Unbekanntes werden mit deutscher Meldung abgelehnt
  parse(input: string): TicketStatus {
    const dashed = input.trim().toLowerCase().replace(/\s+/g, '-');
    if (dashed === 'wartet' || dashed === 'waiting') {
      throw new HarnessError('status-invalid', '„Wartet“ ist abgeleitet aus offenen Voraussetzungen und kein Status.');
    }
    const underscored = dashed.replace(/-/g, '_');
    const direct = TICKET_STATUSES.find((s) => s === underscored);
    const found = direct ?? ALIASES[dashed];
    if (!found) {
      throw new HarnessError('status-invalid', `Status „${input}“ ist ungültig. Erlaubt: ${TICKET_STATUSES.join(', ')}.`);
    }
    return found;
  }
}
