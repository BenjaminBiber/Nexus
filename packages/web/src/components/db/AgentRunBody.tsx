import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type UIEvent } from 'react';
import { createPortal } from 'react-dom';
import { useApi } from '../../api/ApiContext.js';
import type { DispatchRunLogDto, DispatchRunLogPartsDto, DispatchRunShotDto } from '../../api/dispatchTypes.js';
import { groupRunEntries } from '../../dbboard/runGroups.js';
import { compactRunHeader } from '../../dbboard/runLog.js';
import { runLogEntries } from '../../dbboard/runLogEntries.js';
import { useRunTranscript } from '../../hooks/useRunTranscript.js';
import { toolDisplayName } from '../sessions/toolDisplay.js';
import { DispatchLogDialog } from './DispatchLogDialog.js';
import { RunAbortReason } from './run/RunHead.js';
import { RunShotViewer } from './run/RunShotViewer.js';
import { RunThumbs } from './run/RunThumbs.js';
import { RunTranscript } from './run/RunTranscript.js';

/** Näher als so viele Pixel am Ende gilt der Verlauf als „unten“ und bleibt beim Nachladen am Ende */
const STICK_PX = 40;
/** So lange (ms) nach einem Klick im Verlauf gilt eine Größenänderung als Aufklappen durch den Nutzer */
const CLICK_GRACE_MS = 600;

/** Scrollstand eines Verlaufs */
interface Follow {
  /** Stand der Verlauf zuletzt am Ende? Ein frisch geöffneter Lauf beginnt am Ende */
  atEnd: boolean;
  /** Scrollstand nach dem letzten eigenen Sprung ans Ende; -1: seither hat der Nutzer gescrollt */
  jumped: number;
  /** Zeitpunkt des letzten Klicks im Verlauf: Aufklappen soll nicht ans Ende springen */
  clicked: number;
}

/** Steht der Verlauf (fast) am Ende? */
function nearEnd(el: HTMLElement): boolean {
  return el.scrollHeight - el.scrollTop - el.clientHeight < STICK_PX;
}

/** Merkt sich, ob der Verlauf dem Ende folgt. Solange er folgt, bleibt die Scroll-Verankerung des Browsers aus: Wächst
 *  Inhalt über dem Sichtfeld, verschöbe sie den Scrollstand von sich aus – das sähe aus wie Scrollen des Nutzers. */
function setAtEnd(el: HTMLElement, follow: Follow, atEnd: boolean): void {
  follow.atEnd = atEnd;
  el.style.overflowAnchor = atEnd ? 'none' : '';
}

/** Springt ans Ende und merkt sich den Stand: Ein Scroll-Ereignis an genau dieser Stelle stammt vom Sprung selbst */
function toEnd(el: HTMLElement, follow: Follow): void {
  setAtEnd(el, follow, true);
  el.scrollTop = el.scrollHeight;
  follow.jumped = el.scrollTop;
}

interface AgentRunBodyProps {
  run: DispatchRunLogDto;
  /** Protokoll des Laufs: Kopf des Dispatchers, Arbeitsanweisung und Ausgabe (Rückfall ohne Transcript) */
  log: DispatchRunLogPartsDto;
  /** Abstand des Nachladens, solange der Lauf läuft */
  intervalMs?: number;
  /** Platz in der Kopfzeile des Laufs (im <summary>) für die Vorschaubilder des ganzen Laufs; null: keine */
  headSlot?: HTMLElement | null;
}

// Inhalt eines aufgeklappten Laufs: Abbruchgrund, Kopf des Dispatchers, der Verlauf (aus dem Transcript, ohne Transcript
// aus dem Lauf-Protokoll) und im Fuß Arbeitsanweisung und Rohprotokoll. Die Screenshots des Laufs zeichnet er klein in
// die Kopfzeile (`headSlot`), die eines Aufrufs an dessen Zeile; ein Klick öffnet die Großansicht. Der Verlauf bleibt
// am Ende – beim Nachladen und wenn sein Inhalt nachträglich wächst –, solange der Nutzer nicht hochgescrollt oder
// gerade etwas im Verlauf aufgeklappt hat.
export function AgentRunBody({ run, log, intervalMs = 3000, headSlot = null }: AgentRunBodyProps) {
  const api = useApi();
  const tx = useRunTranscript(run.name, run.running, intervalMs);
  // Datei des Screenshots in der Großansicht; null: geschlossen
  const [shot, setShot] = useState<string | null>(null);
  const [raw, setRaw] = useState(false);
  const scroll = useRef<HTMLDivElement | null>(null);
  const follow = useRef<Follow>({ atEnd: true, jumped: -1, clicked: 0 });

  const fallback = !tx.available;
  const logEntries = useMemo(() => runLogEntries(log.body, run.started), [log.body, run.started]);
  const entries = fallback ? logEntries : tx.entries;
  const items = useMemo(() => groupRunEntries(entries, tx.shots, run.running), [entries, tx.shots, run.running]);
  const shotUrl = useCallback((file: string) => api.dispatchRunShotUrl(run.name, file), [api, run.name]);
  const head = useMemo(() => (log.header ? compactRunHeader(log.header) : []), [log.header]);

  // Name des Tools, das einen Screenshot erzeugt hat (Beschriftung der Großansicht); unbekannter Aufruf: leer
  const shotCaption = useCallback(
    (s: DispatchRunShotDto) => {
      const call = entries.find((entry) => entry.kind === 'tool' && entry.id === s.toolUseId);
      return call && call.kind === 'tool' ? toolDisplayName(call.name) : '';
    },
    [entries],
  );

  // Merkt sich bei jedem Scrollen, ob der Verlauf am Ende steht
  const onScroll = (event: UIEvent<HTMLDivElement>) => {
    const el = event.currentTarget;
    // Ereignis des eigenen Sprungs: Ist der Inhalt seither gewachsen, ist der Abstand zum Ende kein Hochscrollen
    if (Math.abs(el.scrollTop - follow.current.jumped) < 1) return;
    follow.current.jumped = -1;
    setAtEnd(el, follow.current, nearEnd(el));
  };

  // Nach dem Zeichnen neuer Einträge wieder ans Ende, wenn der Verlauf dort stand
  useLayoutEffect(() => {
    const el = scroll.current;
    if (el && follow.current.atEnd) toEnd(el, follow.current);
  }, [items, tx.status]);

  // Der Inhalt wächst auch ohne neue Einträge: Einträge außerhalb des Sichtfelds sind nur geschätzt hoch
  // (`content-visibility`) und bekommen ihre echte Höhe erst nach dem Sprung ans Ende, dazu Schrift und Fensterbreite.
  // Stand der Verlauf am Ende, springt er nach. Kurz nach einem Klick gilt das Wachsen als Aufklappen durch den Nutzer:
  // Dann bleibt die Ansicht stehen und es zählt, wo sie jetzt steht.
  const shown = tx.status !== 'loading';
  const empty = items.length === 0;
  useEffect(() => {
    const el = scroll.current;
    if (!shown || !el || typeof ResizeObserver === 'undefined') return undefined;
    const state = follow.current;
    const observer = new ResizeObserver(() => {
      if (Date.now() - state.clicked < CLICK_GRACE_MS) setAtEnd(el, state, nearEnd(el));
      else if (state.atEnd) toEnd(el, state);
    });
    observer.observe(el);
    for (const child of Array.from(el.children)) observer.observe(child);
    return () => observer.disconnect();
  }, [shown, empty]);

  return (
    <>
      {headSlot && createPortal(<RunThumbs shots={tx.shots} all={tx.shots} url={shotUrl} onOpen={setShot} label="Screenshots des Laufs" />, headSlot)}
      <RunAbortReason run={run} />
      {head.length > 0 && (
        <div className="agent-run-head compact">
          {head.map((line, i) => (
            <div key={i}>{line}</div>
          ))}
        </div>
      )}
      {tx.status === 'loading' ? (
        <p className="muted">Wird geladen …</p>
      ) : (
        <>
          {fallback && log.prompt === null && log.body.startsWith('…') && <p className="muted">Nur das Ende des Protokolls – es ist sehr lang.</p>}
          <div className="run-scroll" ref={scroll} onScroll={onScroll} onClickCapture={() => (follow.current.clicked = Date.now())}>
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
      {shot !== null && <RunShotViewer shots={tx.shots} file={shot} url={shotUrl} caption={shotCaption} onChange={setShot} onClose={() => setShot(null)} />}
    </>
  );
}
