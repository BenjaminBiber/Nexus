import { describe, expect, it } from 'vitest';
import { DependencyGraph } from '../../../src/board/DependencyGraph.js';
import type { Ticket, TicketStatus } from '../../../src/board/BoardTypes.js';

// Minimales Ticket für reine Graph-Tests
function ticket(id: string, deps: string[] = [], status: TicketStatus = 'planned', extra: Partial<Ticket> = {}): Ticket {
  return {
    id, projectId: 1, boardId: 1, number: Number(id.slice(-4)), title: `Titel ${id}`, description: '', status,
    priority: 'normal', labels: [], dependsOn: deps, sortOrder: 0, assignee: null, needsPlanning: false, acceptance: '',
    plan: '', handbook: '', diagram: '', spec: null, specTask: null, branch: null, worktree: null, targetBranch: null, touches: [], consumes: [],
    produces: [], blockedReason: null, plannedAt: null, plannedBy: null, planningClaimBy: null, planningClaimAt: null,
    createdAt: '', createdById: '', createdByName: '', updatedAt: '', rev: 1, ...extra,
  };
}

const graph = new DependencyGraph();
const ids = (nodes: Array<{ id: string }>) => nodes.map((n) => n.id).sort();

describe('DependencyGraph.build', () => {
  it('liefert für ein Ticket alle Voraussetzungen und Abhängigen transitiv, auch über Boards', () => {
    const tickets = [
      ticket('ARC-0001'),
      ticket('ARC-0002', ['ARC-0001'], 'planned', { boardId: 2 }),
      ticket('ARC-0003', ['ARC-0002']),
      ticket('ARC-0004', ['ARC-0003'], 'backlog', { boardId: 2 }),
      ticket('ARC-0005'),
    ];
    const g = graph.build(tickets, { kind: 'ticket', id: 'ARC-0003' });
    expect(ids(g.nodes)).toEqual(['ARC-0001', 'ARC-0002', 'ARC-0003', 'ARC-0004']);
    expect(g.edges).toEqual([
      { from: 'ARC-0001', to: 'ARC-0002' }, { from: 'ARC-0002', to: 'ARC-0003' }, { from: 'ARC-0003', to: 'ARC-0004' },
    ]);
    expect(g.nodes.every((n) => !n.external)).toBe(true);
    expect(g.nodes.find((n) => n.id === 'ARC-0004')?.column).toBe('waiting');
  });

  it('nimmt für eine Spec deren Tickets im Projekt und markiert direkte fremde Nachbarn als extern', () => {
    const tickets = [
      ticket('ARC-0001'),
      ticket('ARC-0002', ['ARC-0001'], 'planned', { spec: '001-login' }),
      ticket('ARC-0003', ['ARC-0002'], 'planned', { spec: '001-login', boardId: 2 }),
      ticket('ARC-0004', ['ARC-0003']),
      ticket('ARC-0005', ['ARC-0004']),
      ticket('ARC-0006', [], 'planned', { spec: '001-login', projectId: 2 }),
    ];
    const g = graph.build(tickets, { kind: 'spec', projectId: 1, spec: '001-login' });
    expect(ids(g.nodes)).toEqual(['ARC-0001', 'ARC-0002', 'ARC-0003', 'ARC-0004']);
    expect(g.nodes.filter((n) => n.external).map((n) => n.id).sort()).toEqual(['ARC-0001', 'ARC-0004']);
  });

  it('blendet in der Board-Ansicht Tickets ohne Kante aus und zählt sie', () => {
    const tickets = [
      ticket('ARC-0001'), ticket('ARC-0002', ['ARC-0001']), ticket('ARC-0003'), ticket('ARC-0004'),
      ticket('ARC-0005', ['ARC-0002'], 'planned', { boardId: 2 }),
    ];
    const g = graph.build(tickets, { kind: 'board', boardId: 1 });
    expect(ids(g.nodes)).toEqual(['ARC-0001', 'ARC-0002', 'ARC-0005']);
    expect(g.nodes.find((n) => n.id === 'ARC-0005')?.external).toBe(true);
    expect(g.hiddenWithoutEdges).toBe(2);
  });

  it('blendet Erledigte auf Wunsch aus, das Ticket selbst bleibt', () => {
    const tickets = [ticket('ARC-0001', [], 'done'), ticket('ARC-0002', ['ARC-0001'], 'done'), ticket('ARC-0003', ['ARC-0002'])];
    const g = graph.build(tickets, { kind: 'ticket', id: 'ARC-0002' }, { includeDone: false });
    expect(ids(g.nodes)).toEqual(['ARC-0002', 'ARC-0003']);
    expect(g.edges).toEqual([{ from: 'ARC-0002', to: 'ARC-0003' }]);
    expect(g.hiddenDone).toBe(1);
  });

  it('blendet auf dem Board Tickets aus, deren Kanten nur zu ausgeblendeten Erledigten führen', () => {
    const tickets = [
      ticket('ARC-0001', [], 'done'), ticket('ARC-0002', ['ARC-0001']), ticket('ARC-0003', ['ARC-0001']),
      ticket('ARC-0004'), ticket('ARC-0005', ['ARC-0004']),
    ];
    const g = graph.build(tickets, { kind: 'board', boardId: 1 }, { includeDone: false });
    expect(ids(g.nodes)).toEqual(['ARC-0004', 'ARC-0005']);
    expect(g.hiddenWithoutEdges).toBe(2);
    expect(g.hiddenDone).toBe(1);
  });

  it('erzeugt für unbekannte (gelöschte) Voraussetzungen keine Kante', () => {
    const g = graph.build([ticket('ARC-0001', ['ARC-0099'])], { kind: 'ticket', id: 'ARC-0001' });
    expect(ids(g.nodes)).toEqual(['ARC-0001']);
    expect(g.edges).toEqual([]);
  });

  it('terminiert bei einem Kreis und meldet die schließende Kante', () => {
    const tickets = [ticket('ARC-0001', ['ARC-0002']), ticket('ARC-0002', ['ARC-0001'])];
    const g = graph.build(tickets, { kind: 'ticket', id: 'ARC-0001' });
    expect(ids(g.nodes)).toEqual(['ARC-0001', 'ARC-0002']);
    expect(g.edges).toHaveLength(1);
    expect(g.cycleEdges).toHaveLength(1);
    expect([...g.edges, ...g.cycleEdges]).toEqual(expect.arrayContaining([
      { from: 'ARC-0001', to: 'ARC-0002' }, { from: 'ARC-0002', to: 'ARC-0001' },
    ]));
  });
});

describe('DependencyGraph.criticalPath', () => {
  it('liefert die offene Kette in Reihenfolge der Voraussetzungen', () => {
    const tickets = [ticket('ARC-0001'), ticket('ARC-0002', ['ARC-0001']), ticket('ARC-0003', ['ARC-0002'])];
    expect(graph.build(tickets, { kind: 'board', boardId: 1 }).criticalPath).toEqual(['ARC-0001', 'ARC-0002', 'ARC-0003']);
  });

  it('zählt ein erledigtes Mittelglied nicht und zeigt es nicht', () => {
    const tickets = [ticket('ARC-0001'), ticket('ARC-0002', ['ARC-0001'], 'done'), ticket('ARC-0003', ['ARC-0002'])];
    expect(graph.build(tickets, { kind: 'board', boardId: 1 }).criticalPath).toEqual(['ARC-0001', 'ARC-0003']);
  });

  it('wählt bei gleich langen Ketten das Ziel mit der kleinsten ID', () => {
    const tickets = [ticket('ARC-0001'), ticket('ARC-0004', ['ARC-0001']), ticket('ARC-0002'), ticket('ARC-0003', ['ARC-0002'])];
    expect(graph.build(tickets, { kind: 'board', boardId: 1 }).criticalPath).toEqual(['ARC-0002', 'ARC-0003']);
  });

  it('nimmt in der Ticket-Ansicht die längste Kette in das Ticket', () => {
    const tickets = [
      ticket('ARC-0001'), ticket('ARC-0002', ['ARC-0001']), ticket('ARC-0003', ['ARC-0002']),
      ticket('ARC-0004', ['ARC-0003']), ticket('ARC-0005', ['ARC-0004']),
    ];
    expect(graph.build(tickets, { kind: 'ticket', id: 'ARC-0003' }).criticalPath).toEqual(['ARC-0001', 'ARC-0002', 'ARC-0003']);
  });

  it('ist leer, wenn alles erledigt ist', () => {
    const tickets = [ticket('ARC-0001', [], 'done'), ticket('ARC-0002', ['ARC-0001'], 'not_needed')];
    expect(graph.build(tickets, { kind: 'board', boardId: 1 }).criticalPath).toEqual([]);
  });

  it('rechnet 200 Tickets in einer Kette schnell', () => {
    const tickets = Array.from({ length: 200 }, (_, i) =>
      ticket(`ARC-${String(i + 1).padStart(4, '0')}`, i ? [`ARC-${String(i).padStart(4, '0')}`] : []));
    const start = performance.now();
    const g = graph.build(tickets, { kind: 'board', boardId: 1 });
    expect(performance.now() - start).toBeLessThan(50);
    expect(g.criticalPath).toHaveLength(200);
  });
});

describe('DependencyGraph.unlockedBy', () => {
  it('nennt Abhängige, deren einzige offene Voraussetzung das Ticket ist', () => {
    const tickets = [
      ticket('ARC-0001'), ticket('ARC-0002'),
      ticket('ARC-0003', ['ARC-0001']),
      ticket('ARC-0004', ['ARC-0001', 'ARC-0002']),
      ticket('ARC-0005', ['ARC-0001'], 'in_progress'),
      ticket('ARC-0006', ['ARC-0001', 'ARC-0007'], 'backlog'),
      ticket('ARC-0007', [], 'done'),
    ];
    expect(graph.unlockedBy('ARC-0001', tickets)).toEqual(['ARC-0003', 'ARC-0006']);
  });
});
