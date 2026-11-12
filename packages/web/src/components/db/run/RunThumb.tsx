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

// Vorschaubild eines Screenshots; ein Klick öffnet die Großansicht
export function RunThumb({ shot, url, label, onOpen }: RunThumbProps) {
  return (
    <button type="button" className="run-thumb" aria-label={`${label} vergrößern`} onClick={() => onOpen(shot.file)}>
      <img src={url} alt={label} loading="lazy" />
    </button>
  );
}
