import { useCallback, useLayoutEffect, useMemo, useRef, useState, type UIEvent } from 'react';
import { useApi } from '../../api/ApiContext.js';
import type { DispatchRunLogDto, DispatchRunLogPartsDto } from '../../api/dispatchTypes.js';
import { groupRunEntries } from '../../dbboard/runGroups.js';
import { compactRunHeader } from '../../dbboard/runLog.js';
import { runLogEntries } from '../../dbboard/runLogEntries.js';
import { useRunTranscript } from '../../hooks/useRunTranscript.js';
import { DispatchLogDialog } from './DispatchLogDialog.js';
import { RunAbortReason } from './run/RunHead.js';
import { RunShots } from './run/RunShots.js';
import { RunShotViewer } from './run/RunShotViewer.js';
import { RunTranscript } from './run/RunTranscript.js';

/** Näher als so viele Pixel am Ende gilt der Verlauf als „unten“ und bleibt beim Nachladen am Ende */
const STICK_PX = 40;

interface AgentRunBodyProps {
  run: DispatchRunLogDto;
  /** Protokoll des Laufs: Kopf des Dispatchers, Arbeitsanweisung und Ausgabe (Rückfall ohne Transcript) */
  log: DispatchRunLogPartsDto;
  /** Abstand des Nachladens, solange der Lauf läuft */
  intervalMs?: number;
}

// Inhalt eines aufgeklappten Laufs: Abbruchgrund, Kopf des Dispatchers, Screenshots, der Verlauf (aus dem Transcript,
// ohne Transcript aus dem Lauf-Protokoll) und im Fuß Arbeitsanweisung und Rohprotokoll. Der Verlauf bleibt beim
// Nachladen am Ende, solange der Nutzer nicht hochgescrollt hat.
export function AgentRunBody({ run, log, intervalMs = 3000 }: AgentRunBodyProps) {
  const api = useApi();
  const tx = useRunTranscript(run.name, run.running, intervalMs);
  // Datei des Screenshots in der Großansicht; null: geschlossen
  const [shot, setShot] = useState<string | null>(null);
  const [raw, setRaw] = useState(false);
  const scroll = useRef<HTMLDivElement | null>(null);
  // Stand der Verlauf zuletzt am Ende? Ein frisch geöffneter Lauf beginnt am Ende
  const atEnd = useRef(true);

  const fallback = !tx.available;
  const logEntries = useMemo(() => runLogEntries(log.body, run.started), [log.body, run.started]);
  const entries = fallback ? logEntries : tx.entries;
  const items = useMemo(() => groupRunEntries(entries, tx.shots, run.running), [entries, tx.shots, run.running]);
  const shotUrl = useCallback((file: string) => api.dispatchRunShotUrl(run.name, file), [api, run.name]);

  // Merkt sich bei jedem Scrollen, ob der Verlauf am Ende steht
  const onScroll = (event: UIEvent<HTMLDivElement>) => {
    const el = event.currentTarget;
    atEnd.current = el.scrollHeight - el.scrollTop - el.clientHeight < STICK_PX;
  };

  // Nach dem Zeichnen neuer Einträge wieder ans Ende, wenn der Verlauf dort stand
  useLayoutEffect(() => {
    const el = scroll.current;
    if (el && atEnd.current) el.scrollTop = el.scrollHeight;
  }, [items, tx.status]);

  return (
    <>
      <RunAbortReason run={run} />
      {log.header && (
        <div className="agent-run-head compact">
          {compactRunHeader(log.header).map((line, i) => (
            <div key={i}>{line}</div>
          ))}
        </div>
      )}
      {tx.status === 'loading' ? (
        <p className="muted">Wird geladen …</p>
      ) : (
        <>
          <RunShots shots={tx.shots} url={shotUrl} onOpen={setShot} />
          {fallback && log.prompt === null && log.body.startsWith('…') && <p className="muted">Nur das Ende des Protokolls – es ist sehr lang.</p>}
          <div className="run-scroll" ref={scroll} onScroll={onScroll}>
            <RunTranscript items={items} shots={tx.shots} shotUrl={shotUrl} onShot={setShot} running={run.running} />
          </div>
        </>
      )}
      <div className="run-foot">
        {log.prompt !== null && (
          <details className="agent-prompt">
            <summary className="muted">Arbeitsanweisung an den Agent</summary>
            <pre className="dispatch-log">{log.prompt}</pre>
          </details>
        )}
        <button type="button" className="btn small run-raw" onClick={() => setRaw(true)}>
          Rohprotokoll
        </button>
      </div>
      {raw && <DispatchLogDialog name={run.name} onClose={() => setRaw(false)} />}
      {shot !== null && <RunShotViewer shots={tx.shots} file={shot} url={shotUrl} onChange={setShot} onClose={() => setShot(null)} />}
    </>
  );
}
