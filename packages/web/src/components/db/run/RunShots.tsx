import type { DispatchRunShotDto } from '../../../api/dispatchTypes.js';
import { RunThumb } from './RunThumb.js';

interface RunShotsProps {
  shots: DispatchRunShotDto[];
  /** Adresse eines Screenshots */
  url: (file: string) => string;
  /** Öffnet die Großansicht */
  onOpen: (file: string) => void;
}

// Leiste über dem Verlauf: alle Screenshots des Laufs als Vorschaubilder, dahinter die Anzahl
export function RunShots({ shots, url, onOpen }: RunShotsProps) {
  if (shots.length === 0) return null;
  return (
    <div className="run-shots" role="group" aria-label="Screenshots des Laufs">
      <div className="run-shots-strip">
        {shots.map((shot, index) => (
          <RunThumb key={shot.file} shot={shot} url={url(shot.file)} label={`Screenshot ${index + 1} von ${shots.length}`} onOpen={onOpen} />
        ))}
      </div>
      <span className="muted run-shots-count">{shots.length === 1 ? '1 Screenshot' : `${shots.length} Screenshots`}</span>
    </div>
  );
}
