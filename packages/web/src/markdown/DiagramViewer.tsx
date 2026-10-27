import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { DbModal } from '../components/db/DbModal.js';
import { ZOOM_STEP, naturalSize } from './diagramView.js';
import { usePanZoom } from './usePanZoom.js';

// Ein Mermaid-Diagramm fenstergroß: eingepasst, Zoom über Knöpfe, Mausrad (auch Strg+Mausrad) und +/−/0, Verschieben per Ziehen, Esc schließt.
// Die Hülle (Fokusfalle, Esc, Fokus zurück) kommt von DbModal; gezeigt wird das bereits bereinigte Mermaid-SVG (securityLevel strict).
export function DiagramViewer({ svg, title, onClose }: { svg: string; title: string; onClose: () => void }) {
  const dialog = useRef<HTMLElement | null>(null);
  const titleId = 'dv' + useId().replace(/[^a-zA-Z0-9]/g, '');
  const content = useMemo(() => naturalSize(svg), [svg]);
  const { stage, view, fit, zoomCentered, actualSize, onKeyDown, stageProps } = usePanZoom(content);
  const [fullscreen, setFullscreen] = useState(false);

  // Browser-Vollbild für den Dialog ein- bzw. ausschalten
  const toggleFullscreen = () => {
    if (document.fullscreenElement) void document.exitFullscreen?.();
    else void dialog.current?.requestFullscreen?.();
  };

  // Neu einpassen beim Browser-Vollbild (samt Beschriftung des Knopfs)
  useEffect(() => {
    const onFullscreen = () => {
      setFullscreen(!!document.fullscreenElement);
      fit();
    };
    document.addEventListener('fullscreenchange', onFullscreen);
    return () => document.removeEventListener('fullscreenchange', onFullscreen);
  }, [fit]);

  return (
    <DbModal labelledBy={titleId} onEscape={onClose} className="diagram-viewer" backdropClassName="diagram-viewer-backdrop" dialogRef={dialog} initialFocus={stage}>
      <div className="diagram-viewer-inner" onKeyDown={onKeyDown}>
        <div className="diagram-viewer-bar">
          <strong id={titleId}>{`Diagramm: ${title}`}</strong>
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
          <button type="button" className="btn small" onClick={actualSize}>
            1:1
          </button>
          <button type="button" className="btn small" onClick={toggleFullscreen}>
            {fullscreen ? 'Vollbild beenden' : 'Browser-Vollbild'}
          </button>
          <button type="button" className="btn small" onClick={onClose}>
            Schließen
          </button>
        </div>
        <div tabIndex={0} className="diagram-viewer-stage" {...stageProps}>
          <div
            className="diagram-viewer-canvas"
            style={{ width: content.w, height: content.h, transform: `translate(${view.x}px, ${view.y}px) scale(${view.scale})`, transformOrigin: '0 0' }}
            dangerouslySetInnerHTML={{ __html: svg }}
          />
        </div>
      </div>
    </DbModal>
  );
}
