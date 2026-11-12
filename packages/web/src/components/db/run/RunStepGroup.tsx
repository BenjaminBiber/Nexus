import { useState } from 'react';
import type { DispatchRunShotDto } from '../../../api/dispatchTypes.js';
import type { TranscriptEntry } from '../../../api/sessionDetailTypes.js';
import { entryShots } from '../../../dbboard/runItems.js';
import { TranscriptEntryView } from '../../sessions/TranscriptEntryView.js';
import { RunThumbs } from './RunThumbs.js';

interface RunStepGroupProps {
  /** „7 Schritte · 4× Read · 2× Grep“ */
  label: string;
  entries: TranscriptEntry[];
  /** Screenshots der Aufrufe dieser Gruppe, in der Reihenfolge der Ablage */
  shots: DispatchRunShotDto[];
  /** Alle Screenshots des Laufs – für die Zählung „Screenshot i von n“ */
  all: DispatchRunShotDto[];
  /** Adresse eines Screenshots */
  shotUrl: (file: string) => string;
  /** Öffnet die Großansicht */
  onShot: (file: string) => void;
  /** Vorgabe: offen ist nur die letzte Gruppe eines laufenden Laufs */
  defaultOpen: boolean;
}

// Gefaltete Folge unauffälliger Schritte: eine Zeile mit Anzahl und Tools, aufgeklappt die Einträge selbst. Zugeklappt
// stehen die Screenshots der Schritte klein am rechten Ende der Zeile (neben dem Knopf, nicht darin), aufgeklappt an
// ihren Aufrufen.
export function RunStepGroup({ label, entries, shots, all, shotUrl, onShot, defaultOpen }: RunStepGroupProps) {
  // null: der Nutzer hat nichts gewählt, es gilt die Vorgabe; danach gilt seine Wahl, auch wenn nachgeladen wird
  const [choice, setChoice] = useState<boolean | null>(null);
  const open = choice ?? defaultOpen;
  return (
    <li className="run-group">
      <div className="run-group-row">
        <button type="button" className="run-group-head" aria-expanded={open} onClick={() => setChoice(!open)}>
          <span className="material-icons" aria-hidden="true">
            {open ? 'expand_more' : 'chevron_right'}
          </span>
          <span className="run-group-label">{label}</span>
        </button>
        {!open && <RunThumbs shots={shots} all={all} url={shotUrl} onOpen={onShot} label="Screenshots der Schritte" />}
      </div>
      {open && (
        <ol className="transcript run-group-body">
          {entries.map((entry) => {
            const own = entryShots(entry, shots);
            return (
              <TranscriptEntryView
                key={entry.id}
                entry={entry}
                aside={own.length > 0 ? <RunThumbs shots={own} all={all} url={shotUrl} onOpen={onShot} label="Screenshots des Aufrufs" /> : undefined}
              />
            );
          })}
        </ol>
      )}
    </li>
  );
}
