import type { Ticket, TicketStatus } from './BoardTypes.js';
import { DependencyRules } from './DependencyRules.js';
import { TicketOrdering, type BoardColumn } from './TicketOrdering.js';

// Umfang eines Abhängigkeitsgraphen: ein Ticket, eine Spec im Projekt oder ein Board
export type GraphScope =
  | { kind: 'ticket'; id: string }
  | { kind: 'spec'; projectId: number; spec: string }
  | { kind: 'board'; boardId: number };

export interface GraphOptions {
  /** Erledigte und nicht mehr nötige Tickets zeigen (Standard: an; das Ticket der Ticket-Ansicht bleibt immer) */
  includeDone?: boolean;
}

export interface GraphNode {
  id: string;
  title: string;
  status: TicketStatus;
  column: BoardColumn;
  boardId: number;
  spec: string | null;
  /** Direkter Nachbar außerhalb des Umfangs (andere Spec oder anderes Board) */
  external: boolean;
}

// Kante „to setzt from voraus“
export interface GraphEdge {
  from: string;
  to: string;
}

export interface DependencyGraphResult {
  nodes: GraphNode[];
  edges: GraphEdge[];
  /** Offene Tickets der längsten Kette, von der ersten Voraussetzung bis zum Ziel */
  criticalPath: string[];
  /** Kanten, die einen Kreis schließen; sie fehlen in edges */
  cycleEdges: GraphEdge[];
  /** Tickets des Boards ohne jede Kante (nur Board-Ansicht) */
  hiddenWithoutEdges: number;
  /** Ausgeblendete erledigte oder nicht mehr nötige Tickets */
  hiddenDone: number;
}

const CLOSED: TicketStatus[] = ['done', 'not_needed'];

// Baut Abhängigkeitsgraphen aus Tickets, bricht Kreise auf und bestimmt den kritischen Pfad (reine Funktionen).
export class DependencyGraph {
  private readonly deps = new DependencyRules();
  private readonly ordering = new TicketOrdering();

  // Graph für den Umfang; tickets sind alle bekannten Tickets (über Boards und Projekte)
  build(tickets: Ticket[], scope: GraphScope, options: GraphOptions = {}): DependencyGraphResult {
    const byId = new Map(tickets.map((t) => [t.id, t]));
    const up = new Map<string, string[]>();
    const down = new Map<string, string[]>();
    for (const t of tickets) {
      for (const dep of t.dependsOn) {
        if (!byId.has(dep) || dep === t.id) continue;
        push(up, t.id, dep);
        push(down, dep, t.id);
      }
    }
    const { members, hiddenWithoutEdges } = this.members(tickets, scope, byId, up, down);
    const external = new Set<string>();
    if (scope.kind !== 'ticket') {
      for (const id of members) {
        for (const n of [...(up.get(id) ?? []), ...(down.get(id) ?? [])]) if (!members.has(n)) external.add(n);
      }
    }
    const keep = (id: string) => options.includeDone !== false || !CLOSED.includes(byId.get(id)!.status) ||
      (scope.kind === 'ticket' && id === scope.id);
    const all = [...members, ...external];
    let visible = all.filter(keep).sort();
    let shown = new Set(visible);
    const raw: GraphEdge[] = [];
    for (const to of visible) for (const from of up.get(to) ?? []) if (shown.has(from)) raw.push({ from, to });
    let isolated = 0;
    if (scope.kind === 'board') {
      // Board-Tickets, deren Kanten nur zu ausgeblendeten Erledigten führten, zählen wie kantenlose
      const linked = new Set(raw.flatMap((e) => [e.from, e.to]));
      isolated = visible.filter((id) => !linked.has(id)).length;
      visible = visible.filter((id) => linked.has(id));
      shown = new Set(visible);
    }
    const { edges, cycleEdges } = this.breakCycles(visible, raw);
    const target = scope.kind === 'ticket' ? scope.id : undefined;
    return {
      nodes: visible.map((id) => this.node(byId.get(id)!, byId, external.has(id))),
      edges: sortEdges(edges),
      criticalPath: this.criticalPath(visible.map((id) => byId.get(id)!), edges, target),
      cycleEdges: sortEdges(cycleEdges),
      hiddenWithoutEdges: hiddenWithoutEdges + isolated,
      hiddenDone: all.length - visible.length - isolated,
    };
  }

  // Entfernt per Tiefensuche (iterativ, nach ID sortiert) jede Kante, die zu einem Knoten auf dem aktuellen Weg zurückführt
  breakCycles(nodes: string[], edges: GraphEdge[]): { edges: GraphEdge[]; cycleEdges: GraphEdge[] } {
    const out = new Map<string, GraphEdge[]>();
    for (const e of sortEdges(edges)) push(out, e.from, e);
    const state = new Map<string, 'active' | 'done'>();
    const cycleEdges: GraphEdge[] = [];
    for (const start of [...nodes].sort()) {
      if (state.has(start)) continue;
      const stack: Array<{ id: string; next: number }> = [{ id: start, next: 0 }];
      state.set(start, 'active');
      while (stack.length) {
        const top = stack[stack.length - 1];
        const edge = (out.get(top.id) ?? [])[top.next++];
        if (!edge) {
          state.set(top.id, 'done');
          stack.pop();
          continue;
        }
        const seen = state.get(edge.to);
        if (seen === 'active') cycleEdges.push(edge);
        else if (!seen) {
          state.set(edge.to, 'active');
          stack.push({ id: edge.to, next: 0 });
        }
      }
    }
    const cut = new Set(cycleEdges);
    return { edges: edges.filter((e) => !cut.has(e)), cycleEdges };
  }

  // Längste Kette offener Tickets (Gewicht 1 je offenem Ticket) im kreisfreien Graphen; mit target die längste Kette in
  // dieses Ticket, sonst zum offenen Ticket mit der längsten Kette (Gleichstand: kleinste ID)
  criticalPath(tickets: Ticket[], edges: GraphEdge[], target?: string): string[] {
    const open = (t: Ticket) => !CLOSED.includes(t.status);
    const byId = new Map(tickets.map((t) => [t.id, t]));
    const preds = new Map<string, string[]>();
    const succs = new Map<string, string[]>();
    const indegree = new Map(tickets.map((t) => [t.id, 0]));
    for (const e of edges) {
      if (!byId.has(e.from) || !byId.has(e.to)) continue;
      push(preds, e.to, e.from);
      push(succs, e.from, e.to);
      indegree.set(e.to, indegree.get(e.to)! + 1);
    }
    const queue = [...indegree].filter(([, d]) => d === 0).map(([id]) => id).sort();
    const best = new Map<string, number>();
    const prev = new Map<string, string | null>();
    for (let i = 0; i < queue.length; i++) {
      const id = queue[i];
      let pick: string | null = null;
      for (const p of [...(preds.get(id) ?? [])].sort()) {
        if (pick === null || best.get(p)! > best.get(pick)!) pick = p;
      }
      best.set(id, (pick ? best.get(pick)! : 0) + (open(byId.get(id)!) ? 1 : 0));
      prev.set(id, pick);
      for (const s of succs.get(id) ?? []) {
        indegree.set(s, indegree.get(s)! - 1);
        if (indegree.get(s) === 0) queue.push(s);
      }
    }
    let end: string | undefined = target && best.has(target) ? target : undefined;
    if (!target) {
      for (const t of [...tickets].sort((a, b) => a.id.localeCompare(b.id))) {
        if (open(t) && (end === undefined || best.get(t.id)! > best.get(end)!)) end = t.id;
      }
    }
    const path: string[] = [];
    for (let id: string | null | undefined = end; id; id = prev.get(id)) if (open(byId.get(id)!)) path.unshift(id);
    return path;
  }

  // Abhängige von id, deren einzige offene Voraussetzung id ist und die in Backlog, In Planung oder Geplant stehen
  unlockedBy(id: string, tickets: Ticket[], byId = new Map(tickets.map((t) => [t.id, t]))): string[] {
    return tickets
      .filter((t) => t.dependsOn.includes(id) && ['backlog', 'in_planning', 'planned'].includes(t.status))
      .filter((t) => {
        const open = this.deps.openDependencies(t, byId);
        return open.length === 1 && open[0] === id;
      })
      .map((t) => t.id)
      .sort();
  }

  // Knoten des Umfangs (ohne externe Nachbarn) und Zahl der kantenlos ausgeblendeten Board-Tickets
  private members(tickets: Ticket[], scope: GraphScope, byId: Map<string, Ticket>,
    up: Map<string, string[]>, down: Map<string, string[]>): { members: Set<string>; hiddenWithoutEdges: number } {
    if (scope.kind === 'ticket') {
      if (!byId.has(scope.id)) return { members: new Set(), hiddenWithoutEdges: 0 };
      return { members: new Set([...reach(scope.id, up), ...reach(scope.id, down)]), hiddenWithoutEdges: 0 };
    }
    if (scope.kind === 'spec') {
      return { members: new Set(tickets.filter((t) => t.projectId === scope.projectId && t.spec === scope.spec).map((t) => t.id)), hiddenWithoutEdges: 0 };
    }
    const onBoard = tickets.filter((t) => t.boardId === scope.boardId);
    const linked = onBoard.filter((t) => up.has(t.id) || down.has(t.id));
    return { members: new Set(linked.map((t) => t.id)), hiddenWithoutEdges: onBoard.length - linked.length };
  }

  // Anzeigeknoten mit Spalte (wartet wie auf dem Board)
  private node(t: Ticket, byId: Map<string, Ticket>, external: boolean): GraphNode {
    return {
      id: t.id, title: t.title, status: t.status, column: this.ordering.column(t, this.deps.isWaiting(t, byId)),
      boardId: t.boardId, spec: t.spec, external,
    };
  }
}

// Hängt einen Wert an die Liste des Schlüssels an
function push<T>(map: Map<string, T[]>, key: string, value: T): void {
  const list = map.get(key);
  if (list) list.push(value);
  else map.set(key, [value]);
}

// Alle von start aus erreichbaren IDs (start eingeschlossen), iterativ
function reach(start: string, next: Map<string, string[]>): Set<string> {
  const seen = new Set([start]);
  const stack = [start];
  while (stack.length) {
    for (const n of next.get(stack.pop()!) ?? []) {
      if (!seen.has(n)) {
        seen.add(n);
        stack.push(n);
      }
    }
  }
  return seen;
}

// Kanten nach Ziel, dann Quelle sortiert (deterministische Ausgabe)
function sortEdges(edges: GraphEdge[]): GraphEdge[] {
  return [...edges].sort((a, b) => a.to.localeCompare(b.to) || a.from.localeCompare(b.from));
}
