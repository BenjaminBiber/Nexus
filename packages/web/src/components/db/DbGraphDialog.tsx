import { useId, useMemo, useState, type PointerEvent, type ReactNode } from 'react';
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

// Strich-Icon im 24er-Raster für Kopfzeile und Chips
function Icon({ size, children }: { size: number; children: ReactNode }) {
  return (
    <svg aria-hidden="true" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
      {children}
    </svg>
  );
}

const EyeOff = () => (
  <Icon size={13}><path d="M9.9 4.2A10.4 10.4 0 0 1 12 4c7 0 10 8 10 8a17 17 0 0 1-2.2 3.4" /><path d="M6.6 6.6C3.7 8.6 2 12 2 12s3 8 10 8a9.7 9.7 0 0 0 5.4-1.6" /><path d="M2 2l20 20" /></Icon>
);

// Schlüssel eines Umfangs (für Auswahl und Neuladen)
function keyOf(scope: DbGraphScope): string {
  return scope.kind === 'ticket' ? `t:${scope.id}` : scope.kind === 'board' ? `b:${scope.boardId}` : `s:${scope.project}:${scope.spec}`;
}

// Abhängigkeitsgraph im Dialog: Kopfzeile mit Kennzahlen, Umschalter zwischen Ticket, Spec und Board, Erledigte ein- und ausblenden,
// Zoom/Verschieben und einklappbare Legende.
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
  const { stage, view, fit, zoomCentered, actualSize, onKeyDown, stageProps } = usePanZoom(size);
  const [showLegend, setShowLegend] = useState(true);

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
        <header className="dep-graph-head">
          <div className="dep-graph-heading">
            <div className="dep-graph-name">
              <span className="dep-graph-logo" aria-hidden="true">
                <Icon size={18}><rect x="3" y="3" width="6" height="6" rx="1" /><rect x="15" y="9" width="6" height="6" rx="1" /><rect x="3" y="15" width="6" height="6" rx="1" /><path d="M9 6h3a2 2 0 0 1 2 2v2" /><path d="M9 18h3a2 2 0 0 0 2-2v-2" /></Icon>
              </span>
              <h2 id={titleId}>Abhängigkeiten</h2>
            </div>
            {data && (
              <div className="dep-graph-chips">
                <span className="dep-graph-chip">{data.nodes.length} Tickets</span>
                {data.criticalPath.length > 0 && (
                  <span className="dep-graph-chip is-critical"><span className="dep-graph-chip-line" aria-hidden="true" />Kritischer Pfad: {data.criticalPath.length} offen</span>
                )}
                {data.hiddenWithoutEdges > 0 && (
                  <span className="dep-graph-chip is-quiet" title="Tickets ohne Abhängigkeiten werden nicht angezeigt">
                    <EyeOff />{data.hiddenWithoutEdges} ohne Abhängigkeiten ausgeblendet
                  </span>
                )}
                {data.hiddenDone > 0 && (
                  <span className="dep-graph-chip is-quiet"><EyeOff />{data.hiddenDone} erledigte ausgeblendet</span>
                )}
              </div>
            )}
          </div>
          <div className="dep-graph-tools">
            {scopes.length > 1 && (
              <>
                <div className="segmented" role="group" aria-label="Umfang">
                  {scopes.map((s, i) => (
                    <button key={keyOf(s.scope)} type="button" className={i === chosen ? 'on' : undefined} aria-pressed={i === chosen} onClick={() => choose(i)}>
                      {s.label}
                    </button>
                  ))}
                </div>
                <span className="dep-graph-divider" aria-hidden="true" />
              </>
            )}
            <label className="dep-graph-switch">
              <input type="checkbox" role="switch" checked={showDone} onChange={(e) => setShowDone(e.target.checked)} />
              <span className="dep-graph-switch-track" aria-hidden="true" />
              Erledigte
            </label>
            <span className="dep-graph-divider" aria-hidden="true" />
            <div className="dep-graph-zoom" role="group" aria-label="Zoom">
              <button type="button" className="icon-btn" aria-label="Verkleinern" title="Verkleinern" onClick={() => zoomCentered(1 / ZOOM_STEP)}>
                <Icon size={16}><path d="M5 12h14" /></Icon>
              </button>
              <button type="button" className="dep-graph-zoom-value" aria-label="Zoom auf 100 % zurücksetzen" title="Auf 100 % zurücksetzen" onClick={actualSize}>
                {Math.round(view.scale * 100)} %
              </button>
              <button type="button" className="icon-btn" aria-label="Vergrößern" title="Vergrößern" onClick={() => zoomCentered(ZOOM_STEP)}>
                <Icon size={16}><path d="M12 5v14" /><path d="M5 12h14" /></Icon>
              </button>
              <span className="dep-graph-zoom-divider" aria-hidden="true" />
              <button type="button" className="icon-btn" aria-label="Einpassen" title="Einpassen" onClick={fit}>
                <Icon size={16}><path d="M8 3H5a2 2 0 0 0-2 2v3" /><path d="M21 8V5a2 2 0 0 0-2-2h-3" /><path d="M3 16v3a2 2 0 0 0 2 2h3" /><path d="M16 21h3a2 2 0 0 0 2-2v-3" /></Icon>
              </button>
            </div>
            <span className="dep-graph-divider" aria-hidden="true" />
            <button type="button" className={`icon-btn${showLegend ? ' on' : ''}`} aria-pressed={showLegend} aria-label="Legende ein-/ausblenden" title="Legende" onClick={() => setShowLegend((v) => !v)}>
              <Icon size={18}><circle cx="5" cy="6" r="1.5" /><circle cx="5" cy="12" r="1.5" /><circle cx="5" cy="18" r="1.5" /><path d="M10 6h10" /><path d="M10 12h10" /><path d="M10 18h10" /></Icon>
            </button>
            <button type="button" className="icon-btn" aria-label="Neu laden" title="Neu laden" onClick={() => void graph.reload()}>
              <Icon size={18}><path d="M21 12a9 9 0 1 1-2.6-6.4" /><path d="M21 3v6h-6" /></Icon>
            </button>
            <button type="button" className="icon-btn danger-icon" aria-label="Schließen" title="Schließen" onClick={onClose}>
              <Icon size={18}><path d="M18 6 6 18" /><path d="M6 6l12 12" /></Icon>
            </button>
          </div>
        </header>
        {showLegend && (
          <div className="dep-graph-legend" aria-label="Legende" role="group">
            <ul aria-label="Status">
              <li className="dep-graph-legend-title" aria-hidden="true">Status</li>
              {LEGEND.map((id) => (
                <li key={id} data-status={id}>
                  <span className="column-dot" />
                  {DB_COLUMN_LABELS[id]}
                </li>
              ))}
            </ul>
            <span className="dep-graph-divider" aria-hidden="true" />
            <ul aria-label="Kanten">
              <li className="dep-graph-legend-title" aria-hidden="true">Kanten</li>
              <li className="legend-critical">Kritischer Pfad</li>
              <li className="legend-upstream" title="Beim Zeigen auf ein Ticket">Voraussetzung</li>
              <li className="legend-downstream" title="Beim Zeigen auf ein Ticket">Abhängiges</li>
              <li className="legend-external">Anderes Board / Spec</li>
            </ul>
          </div>
        )}
        {data && data.cycleEdges.length > 0 && (
          <p className="banner dep-graph-state" role="alert">
            Kreis in den Daten: {data.cycleEdges.map((e) => `${e.to} setzt ${e.from} voraus`).join('; ')}. Rot gestrichelt markiert.
          </p>
        )}
        {graph.state.status === 'loading' && <p className="muted dep-graph-state">Graph wird geladen …</p>}
        {graph.state.status === 'error' && <p className="banner dep-graph-state" role="alert">{graph.state.message}</p>}
        {data && data.nodes.length === 0 && <p className="muted dep-graph-state">Keine Tickets mit Abhängigkeiten.</p>}
        <div tabIndex={0} className="diagram-viewer-stage dep-graph-stage" {...stageProps} onPointerDown={onPointerDown} aria-label="Graph verschieben und zoomen">
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
