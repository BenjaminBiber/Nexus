import { Fragment } from 'react';
import type { DispatchRunShotDto } from '../../../api/dispatchTypes.js';
import type { RunItem } from '../../../dbboard/runItems.js';
import { TranscriptEntryView } from '../../sessions/TranscriptEntryView.js';
import { RunStepGroup } from './RunStepGroup.js';
import { RunThumb } from './RunThumb.js';

interface RunTranscriptProps {
  /** Zeilen des Verlaufs (aus `groupRunEntries`) */
  items: RunItem[];
  /** Alle Screenshots des Laufs – für die Zählung „Screenshot i von n“ */
  shots: DispatchRunShotDto[];
  /** Adresse eines Screenshots */
  shotUrl: (file: string) => string;
  /** Öffnet die Großansicht */
  onShot: (file: string) => void;
  running: boolean;
}

// Verlauf eines Agent-Laufs: Einträge wie auf der Sessions-Seite, lange Folgen unauffälliger Schritte gefaltet,
// Screenshots direkt unter ihrem Aufruf
export function RunTranscript({ items, shots, shotUrl, onShot, running }: RunTranscriptProps) {
  if (items.length === 0) {
    return <p className="muted run-empty">{running ? 'Noch keine Ausgabe …' : '(keine Ausgabe)'}</p>;
  }
  const label = (file: string) => `Screenshot ${shots.findIndex((shot) => shot.file === file) + 1} von ${shots.length}`;
  return (
    <ol className="transcript run-transcript">
      {items.map((item) =>
        item.type === 'group' ? (
          <RunStepGroup key={item.key} label={item.label} entries={item.entries} defaultOpen={item.open} />
        ) : (
          <Fragment key={item.key}>
            <TranscriptEntryView entry={item.entry} />
            {item.shots.length > 0 && (
              <li className="run-shot-row">
                {item.shots.map((shot) => (
                  <RunThumb key={shot.file} shot={shot} url={shotUrl(shot.file)} label={label(shot.file)} onOpen={onShot} />
                ))}
              </li>
            )}
          </Fragment>
        ),
      )}
    </ol>
  );
}
