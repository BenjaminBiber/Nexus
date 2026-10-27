import { describe, expect, it } from 'vitest';
import { GAP_X, NODE_HEIGHT, NODE_WIDTH, PADDING, crossings, edgePath, layoutGraph } from '../src/dbboard/graphLayout.js';

describe('layoutGraph', () => {
  it('legt Voraussetzungen links und Abhängige rechts; Schicht = längster Weg', () => {
    const edges = [{ from: 'A', to: 'B' }, { from: 'B', to: 'C' }, { from: 'A', to: 'C' }];
    const l = layoutGraph(['C', 'B', 'A'], edges);
    expect([l.nodes.A.layer, l.nodes.B.layer, l.nodes.C.layer]).toEqual([0, 1, 2]);
    expect(l.nodes.A.x).toBe(PADDING);
    expect(l.nodes.C.x).toBe(PADDING + 2 * (NODE_WIDTH + GAP_X));
    expect(l.width).toBe(2 * PADDING + 3 * NODE_WIDTH + 2 * GAP_X);
    expect(l.height).toBe(2 * PADDING + NODE_HEIGHT);
  });

  it('überlappt keine Knoten', () => {
    const ids = ['A', 'B', 'C', 'D', 'E', 'F'];
    const l = layoutGraph(ids, [{ from: 'A', to: 'D' }, { from: 'B', to: 'D' }, { from: 'C', to: 'E' }]);
    const boxes = ids.map((id) => l.nodes[id]);
    for (let i = 0; i < boxes.length; i++) {
      for (let j = i + 1; j < boxes.length; j++) {
        const a = boxes[i];
        const b = boxes[j];
        const apart = a.x + NODE_WIDTH <= b.x || b.x + NODE_WIDTH <= a.x || a.y + NODE_HEIGHT <= b.y || b.y + NODE_HEIGHT <= a.y;
        expect(apart, `${ids[i]} / ${ids[j]}`).toBe(true);
      }
    }
  });

  it('löst per Baryzentrum eine Kreuzung auf', () => {
    // Nach ID stünde C vor D: C ← B und D ← A kreuzen sich
    const edges = [{ from: 'B', to: 'C' }, { from: 'A', to: 'D' }];
    const naive = { nodes: { A: { x: 0, y: 0, layer: 0, order: 0 }, B: { x: 0, y: 0, layer: 0, order: 1 }, C: { x: 0, y: 0, layer: 1, order: 0 }, D: { x: 0, y: 0, layer: 1, order: 1 } }, width: 0, height: 0 };
    expect(crossings(naive, edges)).toBe(1);
    expect(crossings(layoutGraph(['A', 'B', 'C', 'D'], edges), edges)).toBe(0);
  });

  it('liefert bei gleicher Eingabe dasselbe Ergebnis und kommt mit leerem Graph aus', () => {
    const edges = [{ from: 'A', to: 'B' }, { from: 'X', to: 'B' }];
    expect(layoutGraph(['B', 'A', 'X'], edges)).toEqual(layoutGraph(['X', 'A', 'B'], [...edges].reverse()));
    expect(layoutGraph([], [])).toEqual({ nodes: {}, width: 0, height: 0 });
  });

  it('zeichnet Kanten vom rechten Rand zum linken Rand', () => {
    const l = layoutGraph(['A', 'B'], [{ from: 'A', to: 'B' }]);
    expect(edgePath(l.nodes.A, l.nodes.B)).toMatch(new RegExp(`^M ${PADDING + NODE_WIDTH} ${PADDING + NODE_HEIGHT / 2} C .* ${PADDING + NODE_WIDTH + GAP_X} ${PADDING + NODE_HEIGHT / 2}$`));
  });
});
