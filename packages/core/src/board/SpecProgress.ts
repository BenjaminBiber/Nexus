import type { Ticket } from './BoardTypes.js';
import { DependencyRules } from './DependencyRules.js';

// Stand einer Spec: jedes Ticket in genau einer Gruppe; total ohne „nicht mehr nötig“
export interface SpecProgressCount {
  spec: string;
  total: number;
  done: number;
  blocked: number;
  waiting: number;
  inProgress: number;
  open: number;
  notNeeded: number;
}

// Zählt Tickets je Spec nach Erledigt, Blockiert, Wartet, In Arbeit und Offen (reine Funktion).
export class SpecProgress {
  private readonly deps = new DependencyRules();

  // Zählung je Spec (nach Name sortiert); all löst Voraussetzungen auf (Standard: die gezählten Tickets)
  count(tickets: Ticket[], all: Ticket[] = tickets): SpecProgressCount[] {
    const byId = new Map(all.map((t) => [t.id, t]));
    for (const t of tickets) byId.set(t.id, t);
    const result = new Map<string, SpecProgressCount>();
    for (const t of tickets) {
      if (!t.spec) continue;
      const c = result.get(t.spec) ?? { spec: t.spec, total: 0, done: 0, blocked: 0, waiting: 0, inProgress: 0, open: 0, notNeeded: 0 };
      result.set(t.spec, c);
      if (t.status === 'not_needed') {
        c.notNeeded++;
        continue;
      }
      c.total++;
      if (t.status === 'done') c.done++;
      else if (t.status === 'blocked') c.blocked++;
      else if (this.deps.isWaiting(t, byId)) c.waiting++;
      else if (t.status === 'in_progress') c.inProgress++;
      else c.open++;
    }
    return [...result.values()].sort((a, b) => a.spec.localeCompare(b.spec));
  }
}
