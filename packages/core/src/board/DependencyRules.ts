import { HarnessError } from '../errors/HarnessError.js';
import type { Ticket, TicketStatus } from './BoardTypes.js';
import { TicketIds } from './TicketIds.js';

// Status, in denen ein Ticket als „wartet“ gilt, wenn eine Voraussetzung offen ist
const WAITING_STATUSES: TicketStatus[] = ['backlog', 'in_planning', 'planned'];

// Regeln für Voraussetzungen: Liste bereinigen, Selbstbezug/Unbekannt/Kreis ablehnen, offene ermitteln.
export class DependencyRules {
  private readonly ids = new TicketIds();

  // Trimmt, vereinheitlicht die Schreibweise und entfernt Doppelte (Reihenfolge bleibt)
  normalize(list: string[]): string[] {
    const result: string[] = [];
    for (const raw of list) {
      if (!raw.trim()) continue;
      const id = this.ids.canonical(raw);
      if (!result.includes(id)) result.push(id);
    }
    return result;
  }

  // Prüft die neue Liste gegen alle bekannten Tickets; graph enthält jede ID mit ihren Voraussetzungen
  validate(ticketId: string, deps: string[], graph: Map<string, string[]>): void {
    if (deps.includes(ticketId)) throw new HarnessError('dependency-self', `${ticketId} kann sich nicht selbst voraussetzen.`);
    for (const dep of deps) {
      if (!graph.has(dep)) throw new HarnessError('dependency-unknown', `Voraussetzung ${dep} gibt es nicht.`);
    }
    const next = new Map(graph);
    next.set(ticketId, deps);
    const path = this.cycleFrom(ticketId, next);
    if (path) throw new HarnessError('dependency-cycle', `Abhängigkeiten liefen im Kreis: ${path.join(' -> ')}.`);
  }

  // Voraussetzungen, die weder erledigt noch „nicht mehr nötig“ sind; gelöschte gelten als erfüllt
  openDependencies(ticket: Ticket, byId: Map<string, Ticket>): string[] {
    return ticket.dependsOn.filter((id) => {
      const dep = byId.get(id);
      return dep !== undefined && dep.status !== 'done' && dep.status !== 'not_needed';
    });
  }

  // Erledigte Voraussetzungen, die noch einen Branch tragen (noch nicht gemergt); „nicht mehr nötig“ zählt als erfüllt,
  // weil der Dispatcher solche Tickets nie mergt
  unmergedDependencies(ticket: Ticket, byId: Map<string, Ticket>): string[] {
    return ticket.dependsOn.filter((id) => {
      const dep = byId.get(id);
      return dep !== undefined && dep.status === 'done' && Boolean(dep.branch);
    });
  }

  // Wartet: in Backlog, In Planung oder Geplant mit mindestens einer offenen Voraussetzung
  isWaiting(ticket: Ticket, byId: Map<string, Ticket>): boolean {
    return WAITING_STATUSES.includes(ticket.status) && this.openDependencies(ticket, byId).length > 0;
  }

  // Tiefensuche: Pfad zurück zum Start oder null
  private cycleFrom(start: string, graph: Map<string, string[]>): string[] | null {
    const seen = new Set([start]);
    const visit = (id: string, path: string[]): string[] | null => {
      for (const dep of graph.get(id) ?? []) {
        if (dep === start) return [...path, dep];
        if (seen.has(dep)) continue;
        seen.add(dep);
        const found = visit(dep, [...path, dep]);
        if (found) return found;
      }
      return null;
    };
    return visit(start, [start]);
  }
}
