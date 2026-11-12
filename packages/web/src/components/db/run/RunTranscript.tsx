import type { DispatchRunShotDto } from '../../../api/dispatchTypes.js';
import type { RunItem } from '../../../dbboard/runItems.js';
import { TranscriptEntryView } from '../../sessions/TranscriptEntryView.js';
import { RunStepGroup } from './RunStepGroup.js';
import { RunThumbs } from './RunThumbs.js';

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
// Screenshots klein am rechten Ende der Zeile ihres Aufrufs bzw. ihrer zugeklappten Gruppe
export function RunTranscript({ items, shots, shotUrl, onShot, running }: RunTranscriptProps) {
  if (items.length === 0) {
    return <p className="muted run-empty">{running ? 'Noch keine Ausgabe …' : '(keine Ausgabe)'}</p>;
  }
  return (
    <ol className="transcript run-transcript">
      {items.map((item) =>
        item.type === 'group' ? (
          <RunStepGroup
            key={item.key}
            label={item.label}
            entries={item.entries}
            shots={item.shots}
            all={shots}
            shotUrl={shotUrl}
            onShot={onShot}
            defaultOpen={item.open}
          />
        ) : (
          <TranscriptEntryView
            key={item.key}
            entry={item.entry}
            aside={
              item.shots.length > 0 ? (
                <RunThumbs shots={item.shots} all={shots} url={shotUrl} onOpen={onShot} label="Screenshots des Aufrufs" />
              ) : undefined
            }
          />
        ),
      )}
    </ol>
  );
}
