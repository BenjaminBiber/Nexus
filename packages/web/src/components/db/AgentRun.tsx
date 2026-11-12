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

// Ein Lauf als aufklappbare Zeile: im Kopf Marke, Titel, Dauer, Modell und ganz rechts ein Platz für die Screenshots des
// Laufs, den der offene Inhalt (AgentRunBody) füllt. Nur ein offener Lauf zeichnet seinen Inhalt; zugeklappt lädt er nichts.
export function AgentRun({ run, log, open, intervalMs, onToggle }: AgentRunProps) {
  // Platz für die Vorschaubilder im Kopf; AgentRunBody zeichnet sie per Portal hinein
  const [headSlot, setHeadSlot] = useState<HTMLSpanElement | null>(null);
  return (
    <details className="agent-run" open={open} onToggle={(e) => onToggle(e.currentTarget.open)}>
      <summary>
        <RunHead run={run} />
        <span className="run-head-shots" ref={setHeadSlot} />
      </summary>
      {open && !log && <p className="muted">Wird geladen …</p>}
      {open && log && 'error' in log && <p role="alert" className="field-error">{log.error}</p>}
      {open && log && !('error' in log) && <AgentRunBody run={run} log={log} intervalMs={intervalMs} headSlot={headSlot} />}
    </details>
  );
}
