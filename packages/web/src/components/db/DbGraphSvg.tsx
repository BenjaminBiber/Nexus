import { useId, useMemo, useState, type KeyboardEvent } from 'react';
import { DB_COLUMN_LABELS, type DbGraphDto, type DbGraphNodeDto } from '../../api/dbTypes.js';
import { NODE_HEIGHT, NODE_WIDTH, edgePath, layoutGraph } from '../../dbboard/graphLayout.js';

/** Zeichen des Titels, die in einen Knoten passen */
const TITLE_CHARS = 26;
/** Innenabstand links und rechts im Knoten */
const INSET = 16;
/** Fortschrittsbalken der Schritte am unteren Knotenrand */
const STEPS_BAR = { y: NODE_HEIGHT - 10, width: NODE_WIDTH - 2 * INSET, height: 3 };

// Kürzt einen Titel mit Auslassungszeichen
function short(text: string): string {
  return text.length > TITLE_CHARS ? `${text.slice(0, TITLE_CHARS - 1)}…` : text;
}

// Vorgelesener Fortschritt der Schritte, leer ohne Schritte
function stepsLabel(n: DbGraphNodeDto): string {
  return n.stepsTotal > 0 ? `${n.stepsDone} von ${n.stepsTotal} Schritten erledigt` : '';
}

// Erledigte Schritte als Zahl in der ID-Zeile und als Balken am unteren Rand, wie auf der Board-Karte; ohne Schritte nichts
function StepsProgress({ done, total }: { done: number; total: number }) {
  if (!(total > 0)) return null;
  const complete = done >= total;
  const fill = Math.round((STEPS_BAR.width * Math.min(done, total)) / total);
  return (
    <g className={`dep-graph-steps${complete ? ' is-complete' : ''}`}>
      <text className="dep-graph-steps-text" x={NODE_WIDTH - INSET} y={25} textAnchor="end">{`${done}/${total}`}</text>
      <rect className="dep-graph-steps-track" x={INSET} y={STEPS_BAR.y} width={STEPS_BAR.width} height={STEPS_BAR.height} rx={STEPS_BAR.height / 2} />
      <rect className="dep-graph-steps-fill" x={INSET} y={STEPS_BAR.y} width={fill} height={STEPS_BAR.height} rx={STEPS_BAR.height / 2} />
    </g>
  );
}

// Layout aller Knoten über die kreisfreien Kanten (Kreis-Kanten werden nur gezeichnet); dieselbe Eingabe für Größe und Zeichnung
function layoutOf(graph: DbGraphDto) {
  return layoutGraph(graph.nodes.map((n) => n.id), graph.edges);
}

// Größe des gezeichneten Graphen (für Einpassen und Zoom)
export function graphSize(graph: DbGraphDto): { w: number; h: number } {
  const l = layoutOf(graph);
  return { w: Math.max(1, l.width), h: Math.max(1, l.height) };
}

// Abhängigkeitsgraph als SVG: Knoten in der Statusfarbe des Boards (data-status = Spalte) mit dem Fortschritt ihrer Schritte,
// Kanten „setzt voraus“, kritischer Pfad und Kreis-Kanten hervorgehoben. Knoten sind per Tastatur erreichbar; Klick, Enter oder Leertaste öffnen das Ticket.
// Hover oder Fokus auf einem Knoten markiert seine Voraussetzungs- und Abhängigen-Kanten samt Nachbarn; der Rest tritt zurück.
export function DbGraphSvg({ graph, current, onOpen }: { graph: DbGraphDto; current?: string; onOpen: (id: string) => void }) {
  const layout = useMemo(() => layoutOf(graph), [graph]);
  // Knotenumriss als Clip, damit der Statusstreifen bündig in den runden Ecken sitzt
  const clipId = 'dgc' + useId().replace(/[^a-zA-Z0-9]/g, '');
  const critical = new Set(graph.criticalPath);
  // Knoten unter Maus oder Fokus
  const [hovered, setHovered] = useState<string | null>(null);
  const all = [...graph.edges.map((e) => ({ ...e, kind: 'normal' as const })), ...graph.cycleEdges.map((e) => ({ ...e, kind: 'cycle' as const }))];
  const neighbors = new Set(hovered ? all.flatMap((e) => (e.to === hovered ? [e.from] : e.from === hovered ? [e.to] : [])) : []);
  // Richtung einer Kante zum markierten Knoten: Voraussetzung (zu ihm hin), Abhängiger (von ihm weg) oder unbeteiligt
  const role = (e: { from: string; to: string }) => (!hovered ? null : e.to === hovered ? 'upstream' : e.from === hovered ? 'downstream' : 'dimmed');
  // Markierte Kanten zuletzt zeichnen, damit sie über den anderen liegen
  const ordered = hovered ? [...all].sort((a, b) => Number(role(a) !== 'dimmed') - Number(role(b) !== 'dimmed')) : all;
  // Kante liegt auf dem kritischen Pfad, wenn ihre Enden dort direkt aufeinander folgen
  const onPath = (from: string, to: string) => {
    const at = graph.criticalPath.indexOf(from);
    return at >= 0 && graph.criticalPath[at + 1] === to;
  };

  const onKey = (event: KeyboardEvent<SVGGElement>, id: string) => {
    if (event.key !== 'Enter' && event.key !== ' ') return;
    event.preventDefault();
    onOpen(id);
  };

  const edge = (from: string, to: string, kind: 'normal' | 'cycle') => {
    const a = layout.nodes[from];
    const b = layout.nodes[to];
    if (!a || !b) return null;
    const r = role({ from, to });
    const cls = ['dep-graph-edge', onPath(from, to) ? 'is-critical' : '', kind === 'cycle' ? 'is-cycle' : '', r ? `is-${r}` : ''].filter(Boolean).join(' ');
    const marker = r === 'upstream' || r === 'downstream' ? `dep-graph-arrow-${r}` : 'dep-graph-arrow';
    return <path key={`${kind}:${from}>${to}`} className={cls} d={edgePath(a, b)} markerEnd={`url(#${marker})`} data-edge={`${from}>${to}`} />;
  };

  return (
    <svg className={`dep-graph${hovered ? ' has-hover' : ''}`} width={layout.width} height={layout.height} viewBox={`0 0 ${layout.width} ${layout.height}`} role="group" aria-label="Abhängigkeitsgraph">
      <defs>
        <clipPath id={clipId}>
          <rect width={NODE_WIDTH} height={NODE_HEIGHT} rx={8} />
        </clipPath>
        <marker id="dep-graph-arrow"viewBox="0 0 10 10" refX="10" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
          <path d="M 0 0 L 10 5 L 0 10 z" className="dep-graph-arrow" />
        </marker>
        {(['upstream', 'downstream'] as const).map((r) => (
          <marker key={r} id={`dep-graph-arrow-${r}`} viewBox="0 0 10 10" refX="10" refY="5" markerWidth="3.5" markerHeight="3.5" orient="auto-start-reverse">
            <path d="M 0 0 L 10 5 L 0 10 z" className={`dep-graph-arrow is-${r}`} />
          </marker>
        ))}
      </defs>
      {ordered.map((e) => edge(e.from, e.to, e.kind))}
      {graph.nodes.map((n) => {
        const box = layout.nodes[n.id];
        const cls = ['dep-graph-node', critical.has(n.id) ? 'is-critical' : '', n.external ? 'is-external' : '', n.id === current ? 'is-current' : '',
          n.id === hovered ? 'is-hovered' : hovered ? (neighbors.has(n.id) ? 'is-neighbor' : 'is-dimmed') : '']
          .filter(Boolean).join(' ');
        const where = n.external ? ` – Board ${n.boardName}` : '';
        const steps = stepsLabel(n);
        return (
          <g
            key={n.id}
            className={cls}
            data-status={n.column}
            data-id={n.id}
            transform={`translate(${box.x}, ${box.y})`}
            role="button"
            tabIndex={0}
            aria-label={`${n.id} ${n.title} – ${DB_COLUMN_LABELS[n.column]}${critical.has(n.id) ? ', kritischer Pfad' : ''}${steps ? `, ${steps}` : ''}${where}`}
            onClick={() => onOpen(n.id)}
            onKeyDown={(event) => onKey(event, n.id)}
            onMouseEnter={() => setHovered(n.id)}
            onMouseLeave={() => setHovered((h) => (h === n.id ? null : h))}
            onFocus={() => setHovered(n.id)}
            onBlur={() => setHovered((h) => (h === n.id ? null : h))}
          >
            <title>{`${n.id} ${n.title}\n${DB_COLUMN_LABELS[n.column]} · Board ${n.boardName}${n.spec ? ` · ${n.spec}` : ''}${steps ? `\n${steps}` : ''}`}</title>
            <rect className="dep-graph-box" width={NODE_WIDTH} height={NODE_HEIGHT} rx={8} />
            <rect className="dep-graph-stripe" width={4} height={NODE_HEIGHT} clipPath={`url(#${clipId})`} />
            <text className="dep-graph-id" x={INSET} y={25}>{n.id}</text>
            <text className="dep-graph-title" x={INSET} y={46}>{short(n.title)}</text>
            <StepsProgress done={n.stepsDone} total={n.stepsTotal} />
          </g>
        );
      })}
    </svg>
  );
}
