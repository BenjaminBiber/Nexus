import { useState } from 'react';
import type { DispatchRunLogDto, DispatchRunLogPartsDto } from '../../api/dispatchTypes.js';
import { AgentRunBody } from './AgentRunBody.js';
import { RunHead } from './run/RunHead.js';

interface AgentRunProps {
  run: DispatchRunLogDto;
  /** Protokoll des Laufs; fehlt, solange es lädt, oder ist ein Fehler */
  log: DispatchRunLogPartsDto | { error: string } | undefined;
  open: boolean;
  /** Abstand des Nachladens, solange der Lauf läuft */
  intervalMs: number;
  /** Auf- oder zugeklappt (Nutzer oder Browser) */
  onToggle: (open: boolean) => void;
}

// Ein Lauf als aufklappbare Zeile: im Kopf Marke, Titel, Dauer, Modell; offen dazu die Einstellungen hinter dem Titel und
// ganz rechts Screenshots und „Einklappen“ des Verlaufs, die der offene Inhalt (AgentRunBody) in seine Plätze zeichnet. Nur ein offener Lauf zeichnet seinen Inhalt; zugeklappt lädt er nichts.
export function AgentRun({ run, log, open, intervalMs, onToggle }: AgentRunProps) {
  // Plätze im Kopf für die Einstellungen (hinter dem Titel) sowie Vorschaubilder und „Einklappen“ (ganz rechts);
  // AgentRunBody zeichnet sie per Portal hinein
  const [chipsSlot, setChipsSlot] = useState<HTMLSpanElement | null>(null);
  const [headSlot, setHeadSlot] = useState<HTMLSpanElement | null>(null);
  return (
    <details className="agent-run" open={open} onToggle={(e) => onToggle(e.currentTarget.open)}>
      <summary>
        <RunHead run={run}>
          <span className="run-head-chips" ref={setChipsSlot} />
        </RunHead>
        <span className="run-head-end" ref={setHeadSlot} />
      </summary>
      {open && !log && <p className="muted">Wird geladen …</p>}
      {open && log && 'error' in log && <p role="alert" className="field-error">{log.error}</p>}
      {open && log && !('error' in log) && <AgentRunBody run={run} log={log} intervalMs={intervalMs} chipsSlot={chipsSlot} headSlot={headSlot} />}
    </details>
  );
}
