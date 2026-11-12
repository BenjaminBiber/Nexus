import { useEffect, useId, useRef, useState, type CSSProperties, type MouseEvent, type SyntheticEvent } from 'react';
import type { DispatchRunShotDto } from '../../../api/dispatchTypes.js';
import { formatDateTime } from '../../../sessions/format.js';
import { DbModal } from '../DbModal.js';

interface RunShotViewerProps {
  shots: DispatchRunShotDto[];
  /** Datei des gezeigten Screenshots */
  file: string;
  /** Adresse eines Screenshots */
  url: (file: string) => string;
  /** Beschriftung unten links, z. B. der Name des Tools („Bash“); leer: nur der Zeitpunkt */
  caption?: (shot: DispatchRunShotDto) => string;
  /** Blättert zu einem anderen Screenshot */
  onChange: (file: string) => void;
  onClose: () => void;
}

// Icon eines Knopfs der Großansicht
function Icon({ name }: { name: string }) {
  return (
    <span className="material-icons" aria-hidden="true">
      {name}
    </span>
  );
}

// Großansicht eines Screenshots wie in Claude: dunkle Fläche über das ganze Fenster, Bild mittig und so groß wie möglich,
// Schließen oben rechts, unten links Tool und Zeitpunkt, unten mittig „‹ 2 von 5 ›“. Pfeiltasten blättern, Esc und ein
// Klick auf die dunkle Fläche neben dem Bild schließen. Der Dialog heißt „Screenshot 2 von 5“ (Überschrift nur für
// Hilfstechnik).
export function RunShotViewer({ shots, file, url, caption, onChange, onClose }: RunShotViewerProps) {
  const titleId = useId();
  const closeRef = useRef<HTMLButtonElement | null>(null);
  // Begann der Klick auf der dunklen Fläche? Ein Ziehen vom Bild heraus schließt nicht
  const pressedOnDark = useRef(false);
  // Seitenverhältnis und Breite des geladenen Bildes: damit füllt es die Fläche, ohne dass sein Rahmen über das Bild
  // hinausragt, und wird nie größer gezeigt, als es ist
  const [fit, setFit] = useState<{ file: string; ratio: number; width: number } | null>(null);
  const index = shots.findIndex((shot) => shot.file === file);
  const shot = index >= 0 ? shots[index] : undefined;
  const previous = index > 0 ? shots[index - 1] : undefined;
  const next = index >= 0 ? shots[index + 1] : undefined;

  // Pfeiltasten blättern, solange die Großansicht offen ist
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'ArrowLeft' && previous) onChange(previous.file);
      if (event.key === 'ArrowRight' && next) onChange(next.file);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [previous, next, onChange]);

  // Die dunkle Fläche liegt über dem Hintergrund von DbModal: Drücken und Loslassen direkt auf ihr schließen
  const onDarkDown = (event: MouseEvent<HTMLElement>) => {
    pressedOnDark.current = event.target === event.currentTarget;
  };
  const onDarkClick = (event: MouseEvent<HTMLElement>) => {
    const dark = pressedOnDark.current && event.target === event.currentTarget;
    pressedOnDark.current = false;
    if (dark) onClose();
  };

  // Merkt sich Seitenverhältnis und Breite, sobald das Bild geladen ist
  const onLoad = (event: SyntheticEvent<HTMLImageElement>) => {
    const { naturalWidth, naturalHeight } = event.currentTarget;
    if (naturalWidth > 0 && naturalHeight > 0) setFit({ file, ratio: naturalWidth / naturalHeight, width: naturalWidth });
  };

  if (!shot) return null;
  const label = `Screenshot ${index + 1} von ${shots.length}`;
  const tool = caption?.(shot) ?? '';
  const fitted = fit?.file === shot.file ? fit : null;
  return (
    <DbModal labelledBy={titleId} onEscape={onClose} onBackdrop={onClose} className="run-viewer" backdropClassName="run-viewer-backdrop" initialFocus={closeRef}>
      <div className="run-viewer-surface" onMouseDown={onDarkDown} onClick={onDarkClick}>
        <h2 id={titleId} className="sr-only">
          {label}
        </h2>
        <div className="run-viewer-stage">
          <img
            key={shot.file}
            className={`run-shot-full${fitted ? ' fitted' : ''}`}
            style={fitted ? ({ '--shot-ratio': String(fitted.ratio), '--shot-width': `${fitted.width}px` } as CSSProperties) : undefined}
            src={url(shot.file)}
            alt={label}
            onLoad={onLoad}
          />
        </div>
        <button ref={closeRef} type="button" className="run-viewer-btn run-viewer-close" aria-label="Schließen" onClick={onClose}>
          <Icon name="close" />
        </button>
        <p className="run-viewer-caption">
          {tool && <span className="run-viewer-tool">{tool}</span>}
          <span className="run-viewer-time">{formatDateTime(shot.at)}</span>
        </p>
        {shots.length > 1 && (
          <div className="run-viewer-nav">
            <button type="button" className="run-viewer-btn" aria-label="Vorheriger Screenshot" disabled={!previous} onClick={() => previous && onChange(previous.file)}>
              <Icon name="chevron_left" />
            </button>
            <span className="run-viewer-count">{`${index + 1} von ${shots.length}`}</span>
            <button type="button" className="run-viewer-btn" aria-label="Nächster Screenshot" disabled={!next} onClick={() => next && onChange(next.file)}>
              <Icon name="chevron_right" />
            </button>
          </div>
        )}
      </div>
    </DbModal>
  );
}
