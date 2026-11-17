import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import type { DispatchRunShotDto } from '../../../api/dispatchTypes.js';

interface RunThumbProps {
  shot: DispatchRunShotDto;
  /** Adresse des Bildes (`ApiClient.dispatchRunShotUrl`) */
  url: string;
  /** Alternativtext, z. B. „Screenshot 2 von 5“ */
  label: string;
  /** Öffnet die Großansicht für diese Datei */
  onOpen: (file: string) => void;
}

/** Breite der Vorschau beim Überfahren */
const PREVIEW_WIDTH = 320;
/** Höhe, die über dem Bild frei sein muss, damit die Vorschau darüber steht; sonst darunter */
const PREVIEW_ROOM = 260;
/** So lange liegt der Zeiger auf dem Bild, bevor die Vorschau erscheint */
const PREVIEW_DELAY_MS = 250;
const GAP = 8;

interface PreviewPos {
  left: number;
  top: number;
  above: boolean;
}

// Lage der Vorschau zum Vorschaubild: rechtsbündig am Bild, darüber, wenn Platz ist, sonst darunter; nie aus dem Fenster
function previewPos(rect: DOMRect): PreviewPos {
  const maxLeft = Math.max(GAP, window.innerWidth - PREVIEW_WIDTH - GAP);
  const left = Math.min(Math.max(GAP, rect.right - PREVIEW_WIDTH), maxLeft);
  const above = rect.top >= PREVIEW_ROOM;
  return { left, top: above ? rect.top - GAP : rect.bottom + GAP, above };
}

// Vorschaubild eines Screenshots; ein Klick öffnet die Großansicht. Der Klick klappt kein umgebendes <details> um
// (die Bilder des Laufs stehen in dessen <summary>). Wie in Claude zeigt kurzes Überfahren eine größere Vorschau mit
// der Beschriftung darunter; der Zeiger ist eine Lupe. Die Vorschau hängt an <body>, fängt keine Klicks und
// verschwindet beim Verlassen, Klicken und Scrollen.
export function RunThumb({ shot, url, label, onOpen }: RunThumbProps) {
  const [pos, setPos] = useState<PreviewPos | null>(null);
  const timer = useRef<number | undefined>(undefined);

  const hide = () => {
    window.clearTimeout(timer.current);
    timer.current = undefined;
    setPos(null);
  };

  // Scrollen verschiebt das Bild unter der Vorschau weg; aufräumen beim Aushängen
  useEffect(() => {
    if (!pos) return undefined;
    window.addEventListener('scroll', hide, true);
    return () => window.removeEventListener('scroll', hide, true);
  }, [pos]);
  useEffect(() => () => window.clearTimeout(timer.current), []);

  return (
    <>
      <button
        type="button"
        className="run-thumb"
        aria-label={`${label} vergrößern`}
        onMouseEnter={(event) => {
          const target = event.currentTarget;
          window.clearTimeout(timer.current);
          timer.current = window.setTimeout(() => setPos(previewPos(target.getBoundingClientRect())), PREVIEW_DELAY_MS);
        }}
        onMouseLeave={hide}
        onClick={(event) => {
          event.preventDefault();
          hide();
          onOpen(shot.file);
        }}
      >
        <img src={url} alt={label} loading="lazy" />
      </button>
      {pos &&
        createPortal(
          <div
            className={`run-thumb-preview${pos.above ? ' is-above' : ''}`}
            style={{ left: pos.left, top: pos.top, width: PREVIEW_WIDTH }}
            aria-hidden="true"
          >
            <img src={url} alt="" />
            <span className="run-thumb-preview-label">{label}</span>
          </div>,
          document.body,
        )}
    </>
  );
}
