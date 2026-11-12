import type { DispatchRunShotDto } from '../../../api/dispatchTypes.js';
import { RunThumb } from './RunThumb.js';

/** So viele Vorschaubilder zeigt eine Stelle höchstens; davor steht bei mehr „+N“ */
export const MAX_THUMBS = 3;

interface RunThumbsProps {
  /** Screenshots dieser Stelle (ganzer Lauf oder ein Aufruf), in der Reihenfolge der Ablage */
  shots: DispatchRunShotDto[];
  /** Alle Screenshots des Laufs – für die Zählung „Screenshot i von n“ */
  all: DispatchRunShotDto[];
  /** Adresse eines Screenshots */
  url: (file: string) => string;
  /** Öffnet die Großansicht */
  onOpen: (file: string) => void;
  /** Name der Gruppe für Hilfstechnik, z. B. „Screenshots des Laufs“ */
  label: string;
}

// Kleine Vorschaubilder am rechten Ende einer Zeile wie in Claude: die letzten drei Screenshots, davor bei mehr ein
// Knopf „+N“, der die Großansicht beim ersten Screenshot dieser Stelle öffnet. Klicks klappen kein <details> um.
export function RunThumbs({ shots, all, url, onOpen, label }: RunThumbsProps) {
  if (shots.length === 0) return null;
  const visible = shots.slice(-MAX_THUMBS);
  const hidden = shots.length - visible.length;
  const count = (file: string) => `Screenshot ${all.findIndex((shot) => shot.file === file) + 1} von ${all.length}`;
  return (
    <span className="run-thumbs" role="group" aria-label={label}>
      {hidden > 0 && (
        <button
          type="button"
          className="run-thumbs-more"
          aria-label={hidden === 1 ? '1 weiterer Screenshot ansehen' : `${hidden} weitere Screenshots ansehen`}
          onClick={(event) => {
            event.preventDefault();
            onOpen(shots[0]!.file);
          }}
        >
          {`+${hidden}`}
        </button>
      )}
      {visible.map((shot) => (
        <RunThumb key={shot.file} shot={shot} url={url(shot.file)} label={count(shot.file)} onOpen={onOpen} />
      ))}
    </span>
  );
}
