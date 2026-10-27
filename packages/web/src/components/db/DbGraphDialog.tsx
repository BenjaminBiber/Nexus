import { useId, useMemo, useState, type PointerEvent } from 'react';
import { useApi } from '../../api/ApiContext.js';
import { DB_COLUMN_LABELS, type DbColumnId, type DbGraphScope } from '../../api/dbTypes.js';
import { useAsync } from '../../hooks/useAsync.js';
import { ZOOM_STEP } from '../../markdown/diagramView.js';
import { usePanZoom } from '../../markdown/usePanZoom.js';
import { DbGraphSvg, graphSize } from './DbGraphSvg.js';
import { DbModal } from './DbModal.js';

/** Ein wählbarer Umfang im Umschalter */
export interface GraphScopeOption { label: string; scope: DbGraphScope }

const LEGEND: DbColumnId[] = ['backlog', 'waiting', 'in_planning', 'planned', 'in_progress', 'blocked', 'done', 'not_needed'];

// Schlüssel eines Umfangs (für Auswahl und Neuladen)
function keyOf(scope: DbGraphScope): string {
  return scope.kind === 'ticket' ? `t:${scope.id}` : scope.kind === 'board' ? `b:${scope.boardId}` : `s:${scope.project}:${scope.spec}`;
}

// Abhängigkeitsgraph im Dialog: Umschalter zwischen Ticket, Spec und Board, Legende, Zoom/Verschieben, Erledigte ein- und ausblenden.
// Ein Klick auf einen Knoten öffnet das Ticket über onOpenTicket.
export function DbGraphDialog({ scopes, initial = 0, onClose, onOpenTicket }: {
  scopes: GraphScopeOption[];
  initial?: number;
  onClose: () => void;
  onOpenTicket: (id: string) => void;
}) {
  const api = useApi();
  const titleId = 'dg' + useId().replace(/[^a-zA-Z0-9]/g, '');
  const [chosen, setChosen] = useState(Math.min(initial, scopes.length - 1));
  const scope = scopes[chosen].scope;
  const [showDone, setShowDone] = useState(scope.kind !== 'board');
  const key = keyOf(scope);
  const graph = useAsync(() => api.getDbGraph(scope, showDone), [api, key, showDone]);
  const data = graph.state.status === 'ready' ? graph.state.data : null;
  const size = useMemo(() => (data ? graphSize(data) : { w: 1, h: 1 }), [data]);
  const { stage, view, fit, zoomCentered, onKeyDown, stageProps } = usePanZoom(size);

  // Beim Wechsel des Umfangs gilt dessen Standard für „Erledigte zeigen“
  const choose = (index: number) => {
    setChosen(index);
    setShowDone(scopes[index].scope.kind !== 'board');
  };

  // Ziehen nur auf freier Fläche; auf einem Knoten bleibt der Klick beim Knoten
  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if ((event.target as Element).closest?.('.dep-graph-node')) return;
    stageProps.onPointerDown(event);
  };

  const current = scope.kind === 'ticket' ? scope.id : undefined;

  return (
    <DbModal labelledBy={titleId} onEscape={onClose} className="diagram-viewer dep-graph-dialog" backdropClassName="diagram-viewer-backdrop" initialFocus={stage}>
      <div className="diagram-viewer-inner" onKeyDown={onKeyDown}>
        <div className="diagram-viewer-bar">
          <strong id={titleId}>Abhängigkeiten</strong>
          {scopes.length > 1 && (
            <div className="dep-graph-scopes" role="group" aria-label="Umfang">
              {scopes.map((s, i) => (
                <button key={keyOf(s.scope)} type="button" className={`btn small${i === chosen ? ' primary' : ''}`} aria-pressed={i === chosen} onClick={() => choose(i)}>
                  {s.label}
                </button>
              ))}
            </div>
          )}
          <label className="inline-field">
            <input type="checkbox" checked={showDone} onChange={(e) => setShowDone(e.target.checked)} />
            Erledigte zeigen
          </label>
          <button type="button" className="btn small" onClick={() => zoomCentered(1 / ZOOM_STEP)}>
            Verkleinern
          </button>
          <span className="mono">{Math.round(view.scale * 100)} %</span>
          <button type="button" className="btn small" onClick={() => zoomCentered(ZOOM_STEP)}>
            Vergrößern
          </button>
          <button type="button" className="btn small" onClick={fit}>
            Einpassen
          </button>
          <button type="button" className="btn small" onClick={() => void graph.reload()}>
            Neu laden
          </button>
          <button type="button" className="btn small" onClick={onClose}>
            Schließen
          </button>
        </div>
        <div className="dep-graph-info">
          <ul className="dep-graph-legend" aria-label="Legende">
            {LEGEND.map((id) => (
              <li key={id} data-status={id}>
                <span className="column-dot" />
                {DB_COLUMN_LABELS[id]}
              </li>
            ))}
            <li className="legend-critical">kritischer Pfad</li>
            <li className="legend-upstream" title="Beim Zeigen auf ein Ticket">Voraussetzung</li>
            <li className="legend-downstream" title="Beim Zeigen auf ein Ticket">Abhängiges</li>
            <li className="legend-external">anderes Board / andere Spec</li>
          </ul>
          {data && (
            <p className="muted">
              {data.nodes.length} Tickets
              {data.criticalPath.length > 0 && `, kritischer Pfad: ${data.criticalPath.length} offen`}
              {data.hiddenWithoutEdges > 0 && `, ${data.hiddenWithoutEdges} ohne Abhängigkeiten ausgeblendet`}
              {data.hiddenDone > 0 && `, ${data.hiddenDone} erledigte ausgeblendet`}
            </p>
          )}
          {data && data.cycleEdges.length > 0 && (
            <p className="banner" role="alert">
              Kreis in den Daten: {data.cycleEdges.map((e) => `${e.to} setzt ${e.from} voraus`).join('; ')}. Rot gestrichelt markiert.
            </p>
          )}
        </div>
        {graph.state.status === 'loading' && <p className="muted dep-graph-state">Graph wird geladen …</p>}
        {graph.state.status === 'error' && <p className="banner dep-graph-state" role="alert">{graph.state.message}</p>}
        {data && data.nodes.length === 0 && <p className="muted dep-graph-state">Keine Tickets mit Abhängigkeiten.</p>}
        <div tabIndex={0} className="diagram-viewer-stage" {...stageProps} onPointerDown={onPointerDown} aria-label="Graph verschieben und zoomen">
          {data && data.nodes.length > 0 && (
            <div className="diagram-viewer-canvas" style={{ width: size.w, height: size.h, transform: `translate(${view.x}px, ${view.y}px) scale(${view.scale})`, transformOrigin: '0 0' }}>
              <DbGraphSvg
                graph={data}
                current={current}
                onOpen={onOpenTicket}
              />
            </div>
          )}
        </div>
      </div>
    </DbModal>
  );
}
