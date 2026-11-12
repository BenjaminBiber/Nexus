import { useState } from 'react';
import type { TranscriptEntry } from '../../../api/sessionDetailTypes.js';
import { TranscriptEntryView } from '../../sessions/TranscriptEntryView.js';

interface RunStepGroupProps {
  /** „7 Schritte · 4× Read · 2× Grep“ */
  label: string;
  entries: TranscriptEntry[];
  /** Vorgabe: offen ist nur die letzte Gruppe eines laufenden Laufs */
  defaultOpen: boolean;
}

// Gefaltete Folge unauffälliger Schritte: eine Zeile mit Anzahl und Tools, aufgeklappt die Einträge selbst
export function RunStepGroup({ label, entries, defaultOpen }: RunStepGroupProps) {
  // null: der Nutzer hat nichts gewählt, es gilt die Vorgabe; danach gilt seine Wahl, auch wenn nachgeladen wird
  const [choice, setChoice] = useState<boolean | null>(null);
  const open = choice ?? defaultOpen;
  return (
    <li className="run-group">
      <button type="button" className="run-group-head" aria-expanded={open} onClick={() => setChoice(!open)}>
        <span className="material-icons" aria-hidden="true">
          {open ? 'expand_more' : 'chevron_right'}
        </span>
        <span className="run-group-label">{label}</span>
      </button>
      {open && (
        <ol className="transcript run-group-body">
          {entries.map((entry) => (
            <TranscriptEntryView key={entry.id} entry={entry} />
          ))}
        </ol>
      )}
    </li>
  );
}
