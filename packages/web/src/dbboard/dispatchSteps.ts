import type { DispatchTicketRowDto } from '../api/dispatchTypes.js';

// Schritt der Geplant-Liste: Tickets, deren Voraussetzungen in früheren Schritten liegen
export interface DispatchStep {
  number: number;
  tickets: DispatchTicketRowDto[];
  // Mindestens ein Ticket des Schritts ist bereit
  next: boolean;
}

// Voraussetzungen laut Grund („Wartet auf A, B“ oder „wartet auf Merge von A“); Fragen zählen nicht
export function waitsOn(reason: string): string[] {
  const match = /^Wartet auf (?!Frage )(.+)$/.exec(reason) ?? /^wartet auf Merge von (.+)$/.exec(reason);
  return match ? match[1]!.split(',').map((id) => id.trim()).filter(Boolean) : [];
}

export function isReady(reason: string): boolean {
  return reason.startsWith('bereit');
}

// Ordnet die Geplant-Tickets nach ihren Abhängigkeiten untereinander in Schritte (Reihenfolge innerhalb eines Schritts bleibt)
export function dispatchSteps(tickets: DispatchTicketRowDto[]): DispatchStep[] {
  const byId = new Map(tickets.map((t) => [t.id, t]));
  const depth = new Map<string, number>();
  const visit = (id: string, seen: Set<string>): number => {
    const known = depth.get(id);
    if (known !== undefined) return known;
    // Zyklus: das Ticket bleibt im frühesten Schritt
    if (seen.has(id)) return 0;
    seen.add(id);
    const deps = waitsOn(byId.get(id)!.reason).filter((dep) => byId.has(dep));
    const level = deps.length ? 1 + Math.max(...deps.map((dep) => visit(dep, seen))) : 0;
    depth.set(id, level);
    return level;
  };
  const steps: DispatchStep[] = [];
  for (const ticket of tickets) {
    const level = visit(ticket.id, new Set());
    for (let i = steps.length; i <= level; i++) steps.push({ number: i + 1, tickets: [], next: false });
    steps[level]!.tickets.push(ticket);
    if (isReady(ticket.reason)) steps[level]!.next = true;
  }
  return steps.filter((s) => s.tickets.length).map((s, i) => ({ ...s, number: i + 1 }));
}

// Wie viele Geplant-Tickets (auch mittelbar) auf das Ticket warten
export function unlocks(ticketId: string, tickets: DispatchTicketRowDto[]): number {
  const freed = new Set<string>();
  const queue = [ticketId];
  while (queue.length) {
    const id = queue.shift()!;
    for (const t of tickets) {
      if (!freed.has(t.id) && waitsOn(t.reason).includes(id)) {
        freed.add(t.id);
        queue.push(t.id);
      }
    }
  }
  return freed.size;
}
