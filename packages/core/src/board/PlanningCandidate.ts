import { PLANNING_CLAIM_TTL_MS, type Ticket } from './BoardTypes.js';

// Entscheidet, ob ein Ticket für den nächsten Planungslauf in Frage kommt.
export class PlanningCandidate {
  // Planungsbedarf, nicht In Planung/Nicht mehr nötig, keine offene Frage zum Ticket und kein lebender Claim.
  // Der Blockiert-Grund „Rückfrage“ hat keine Sonderrolle mehr: Rückfragen sind Fragen auf der Board-Datenbank.
  static matches(ticket: Ticket, now: number, openQuestions = 0): boolean {
    return ticket.needsPlanning && ticket.status !== 'in_planning' && ticket.status !== 'not_needed' && openQuestions === 0 &&
      !(!!ticket.planningClaimAt && now - Date.parse(ticket.planningClaimAt) < PLANNING_CLAIM_TTL_MS);
  }
}
