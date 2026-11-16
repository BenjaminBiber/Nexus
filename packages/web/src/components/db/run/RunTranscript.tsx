import type { DispatchRunShotDto } from '../../../api/dispatchTypes.js';
import type { RunItem } from '../../../dbboard/runItems.js';
import { TranscriptEntryView } from '../../sessions/TranscriptEntryView.js';
import { RunStepGroup } from './RunStepGroup.js';
import { RunThumbs } from './RunThumbs.js';

interface RunTranscriptProps {
  /** Zeilen des Verlaufs (aus `groupRunEntries`) */
  items: RunItem[];
  /** Erste Zeile „Claude startet (Modell)“; null: keine */
  start?: string | null;
  /** Alle Screenshots des Laufs – für die Zählung „Screenshot i von n“ */
  shots: DispatchRunShotDto[];
  /** Adresse eines Screenshots */
  shotUrl: (file: string) => string;
  /** Öffnet die Großansicht */
  onShot: (file: string) => void;
  running: boolean;
}

// Verlauf eines Agent-Laufs als Zeitleiste: links eine Linie mit einem Punkt je Zeile, oben der Start von Claude, darunter
// Zwischenstände des Agenten, gefaltete Folgen unauffälliger Schritte und einzelne Aufrufe (der letzte hervorgehoben).
// Screenshots stehen klein am rechten Ende der Zeile ihres Aufrufs bzw. ihrer zugeklappten Gruppe.
export function RunTranscript({ items, start = null, shots, shotUrl, onShot, running }: RunTranscriptProps) {
  const empty = <span className="muted run-empty">{running ? 'Noch keine Ausgabe …' : '(keine Ausgabe)'}</span>;
  if (items.length === 0 && start === null) return <p className="run-empty-line">{empty}</p>;
  // Letzter Schritt: der letzte einzelne Aufruf, wenn hinter ihm keine Gruppe mehr steht
  const steps = items.filter((item) => item.type === 'group' || item.entry.kind === 'tool');
  const lastStep = steps.at(-1);
  const lastKey = lastStep?.type === 'entry' ? lastStep.key : null;
  return (
    <ol className="transcript run-transcript run-track">
      {start !== null && <li className="run-start">{start}</li>}
      {items.map((item) =>
        item.type === 'group' ? (
          <RunStepGroup
            key={item.key}
            label={item.label}
            tools={item.tools}
            breakdown={item.breakdown}
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
            className={item.key === lastKey ? 'run-last' : undefined}
            aside={
              item.shots.length > 0 ? (
                <RunThumbs shots={item.shots} all={shots} url={shotUrl} onOpen={onShot} label="Screenshots des Aufrufs" />
              ) : undefined
            }
          />
        ),
      )}
      {items.length === 0 && <li className="run-start">{empty}</li>}
    </ol>
  );
}
