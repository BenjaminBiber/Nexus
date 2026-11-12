import { useEffect, useId } from 'react';
import type { DispatchRunShotDto } from '../../../api/dispatchTypes.js';
import { formatDateTime } from '../../../sessions/format.js';
import { DbModal } from '../DbModal.js';

interface RunShotViewerProps {
  shots: DispatchRunShotDto[];
  /** Datei des gezeigten Screenshots */
  file: string;
  /** Adresse eines Screenshots */
  url: (file: string) => string;
  /** Blättert zu einem anderen Screenshot */
  onChange: (file: string) => void;
  onClose: () => void;
}

// Großansicht eines Screenshots als Dialog: Bild, Zählung, Zeitpunkt, Blättern mit Knöpfen und Pfeiltasten
export function RunShotViewer({ shots, file, url, onChange, onClose }: RunShotViewerProps) {
  const titleId = useId();
  const index = shots.findIndex((shot) => shot.file === file);
  const shot = index >= 0 ? shots[index] : undefined;
  const previous = index > 0 ? shots[index - 1] : undefined;
  const next = index >= 0 ? shots[index + 1] : undefined;

  // Pfeiltasten blättern, solange der Dialog offen ist
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'ArrowLeft' && previous) onChange(previous.file);
      if (event.key === 'ArrowRight' && next) onChange(next.file);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [previous, next, onChange]);

  if (!shot) return null;
  const label = `Screenshot ${index + 1} von ${shots.length}`;
  return (
    <DbModal labelledBy={titleId} onEscape={onClose} onBackdrop={onClose} className="panel db-modal wide run-shot-viewer">
      <div className="db-section-head">
        <h2 id={titleId}>{label}</h2>
        <span className="muted">{formatDateTime(shot.at)}</span>
        <button type="button" className="btn" onClick={onClose}>
          Schließen
        </button>
      </div>
      <img className="run-shot-full" src={url(shot.file)} alt={label} />
      <div className="run-shot-nav">
        <button type="button" className="btn" disabled={!previous} onClick={() => previous && onChange(previous.file)}>
          Zurück
        </button>
        <button type="button" className="btn" disabled={!next} onClick={() => next && onChange(next.file)}>
          Weiter
        </button>
      </div>
    </DbModal>
  );
}
