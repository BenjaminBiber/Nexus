// Schichtenlayout für Abhängigkeitsgraphen (reine Funktionen): Voraussetzungen links, Abhängige rechts.
// Schicht = längster Weg von einer Quelle; Reihenfolge je Schicht per Baryzentrum (4 Durchläufe), feste Knotengröße.

export interface LayoutEdge { from: string; to: string }
export interface NodeBox { x: number; y: number; layer: number; order: number }
export interface GraphLayout {
  nodes: Record<string, NodeBox>;
  width: number;
  height: number;
}

export const NODE_WIDTH = 230;
export const NODE_HEIGHT = 64;
export const GAP_X = 56;
export const GAP_Y = 26;
export const PADDING = 16;
const SWEEPS = 4;

// Ordnet die Knoten in Schichten an; Kanten müssen kreisfrei sein (unbekannte Enden werden übergangen)
export function layoutGraph(ids: string[], edges: LayoutEdge[]): GraphLayout {
  const known = new Set(ids);
  const valid = edges.filter((e) => known.has(e.from) && known.has(e.to) && e.from !== e.to);
  const preds = new Map<string, string[]>(ids.map((id) => [id, []]));
  const succs = new Map<string, string[]>(ids.map((id) => [id, []]));
  for (const e of valid) {
    preds.get(e.to)!.push(e.from);
    succs.get(e.from)!.push(e.to);
  }
  const layerOf = layers([...ids].sort(), preds, succs);
  const count = Math.max(0, ...layerOf.values()) + 1;
  const rows: string[][] = Array.from({ length: ids.length ? count : 0 }, () => []);
  for (const id of [...ids].sort()) rows[layerOf.get(id)!].push(id);
  for (let sweep = 0; sweep < SWEEPS; sweep++) {
    if (sweep % 2 === 0) for (let i = 1; i < rows.length; i++) rows[i] = byBarycenter(rows[i], rows[i - 1], preds);
    else for (let i = rows.length - 2; i >= 0; i--) rows[i] = byBarycenter(rows[i], rows[i + 1], succs);
  }
  const tallest = Math.max(0, ...rows.map((r) => r.length));
  const nodes: Record<string, NodeBox> = {};
  rows.forEach((row, layer) => {
    const offset = ((tallest - row.length) * (NODE_HEIGHT + GAP_Y)) / 2;
    row.forEach((id, order) => {
      nodes[id] = { x: PADDING + layer * (NODE_WIDTH + GAP_X), y: PADDING + offset + order * (NODE_HEIGHT + GAP_Y), layer, order };
    });
  });
  return {
    nodes,
    width: rows.length ? 2 * PADDING + rows.length * NODE_WIDTH + (rows.length - 1) * GAP_X : 0,
    height: tallest ? 2 * PADDING + tallest * NODE_HEIGHT + (tallest - 1) * GAP_Y : 0,
  };
}

// SVG-Pfad einer Kante: vom rechten Rand der Voraussetzung zum linken Rand des Abhängigen (kubische Kurve)
export function edgePath(from: NodeBox, to: NodeBox): string {
  const x1 = from.x + NODE_WIDTH;
  const y1 = from.y + NODE_HEIGHT / 2;
  const x2 = to.x;
  const y2 = to.y + NODE_HEIGHT / 2;
  const bend = Math.max(24, Math.abs(x2 - x1) / 2);
  return `M ${x1} ${y1} C ${x1 + bend} ${y1}, ${x2 - bend} ${y2}, ${x2} ${y2}`;
}

// Kreuzungen zwischen Kanten benachbarter Schichten (zum Prüfen der Reihenfolge)
export function crossings(layout: GraphLayout, edges: LayoutEdge[]): number {
  const adjacent = edges
    .map((e) => ({ a: layout.nodes[e.from], b: layout.nodes[e.to] }))
    .filter((e) => e.a && e.b && e.b.layer === e.a.layer + 1);
  let total = 0;
  for (let i = 0; i < adjacent.length; i++) {
    for (let j = i + 1; j < adjacent.length; j++) {
      const p = adjacent[i];
      const q = adjacent[j];
      if (p.a.layer !== q.a.layer) continue;
      if ((p.a.order - q.a.order) * (p.b.order - q.b.order) < 0) total++;
    }
  }
  return total;
}

// Schicht je Knoten: längster Weg von einer Quelle (topologische Reihenfolge nach Kahn)
function layers(ids: string[], preds: Map<string, string[]>, succs: Map<string, string[]>): Map<string, number> {
  const indegree = new Map(ids.map((id) => [id, preds.get(id)!.length]));
  const layer = new Map(ids.map((id) => [id, 0]));
  const queue = ids.filter((id) => indegree.get(id) === 0);
  for (let i = 0; i < queue.length; i++) {
    const id = queue[i];
    for (const s of succs.get(id)!) {
      layer.set(s, Math.max(layer.get(s)!, layer.get(id)! + 1));
      indegree.set(s, indegree.get(s)! - 1);
      if (indegree.get(s) === 0) queue.push(s);
    }
  }
  return layer;
}

// Sortiert eine Schicht nach dem Mittel der Positionen ihrer Nachbarn in der Bezugsschicht; ohne Nachbarn bleibt die Position
function byBarycenter(row: string[], reference: string[], neighbours: Map<string, string[]>): string[] {
  const pos = new Map(reference.map((id, i) => [id, i]));
  const key = new Map(row.map((id, i) => {
    const near = neighbours.get(id)!.map((n) => pos.get(n)).filter((p): p is number => p !== undefined);
    return [id, near.length ? near.reduce((s, p) => s + p, 0) / near.length : i];
  }));
  return [...row].sort((a, b) => key.get(a)! - key.get(b)! || row.indexOf(a) - row.indexOf(b));
}
