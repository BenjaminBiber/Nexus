import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { TranscriptState } from '../../hooks/useSessionDetail.js';
import { formatNumber } from '../../sessions/format.js';
import { EmptyState } from '../EmptyState.js';
import { ErrorState } from '../ErrorState.js';
import { LoadingState } from '../LoadingState.js';
import { TranscriptEntryView } from './TranscriptEntryView.js';

// Abstand zum Ende (px), bis zu dem die Liste als „unten“ gilt
const BOTTOM_SLACK = 48;
// So lange (ms) nach einem Klick gilt Größenänderung als Aufklappen durch den Nutzer
const CLICK_GRACE_MS = 600;

interface Props {
  transcript: TranscriptState;
  /** Tab sichtbar: beim Einblenden ans Ende springen, wenn der Nutzer dort war */
  active: boolean;
  /** Session läuft gerade (Hinweis am Ende) */
  running: boolean;
  onRetry: () => void;
}

// Verlauf einer Session als Chat. Folgt dem Ende: Ist der Nutzer unten, scrollt die Liste bei neuen Einträgen mit;
// sonst erscheint „Neue Einträge ↓“.
export function SessionTranscript({ transcript, active, running, onRetry }: Props) {
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const atBottom = useRef(true);
  const seen = useRef(0);
  // Zeitpunkt des letzten Klicks im Verlauf: Aufklappen soll nicht ans Ende springen
  const clicked = useRef(0);
  const [unseen, setUnseen] = useState(0);
  const { entries } = transcript;

  const toEnd = (smooth = false) => {
    const node = scrollRef.current;
    if (!node) return;
    if (smooth && typeof node.scrollTo === 'function') node.scrollTo({ top: node.scrollHeight, behavior: 'smooth' });
    else node.scrollTop = node.scrollHeight;
  };

  // Neue oder ersetzte Einträge: unten bleiben oder neue zählen
  useLayoutEffect(() => {
    const added = entries.length - seen.current;
    seen.current = entries.length;
    if (atBottom.current) toEnd();
    else if (added > 0) setUnseen((n) => n + added);
  }, [entries]);

  // Beim Einblenden des Tabs stimmen die Maße erst jetzt
  useEffect(() => {
    if (active && atBottom.current) toEnd();
  }, [active]);

  // Inhalt wächst auch ohne neue Einträge (Schrift, Markdown, Aufklappen, Fensterbreite): unten bleiben
  const shown = transcript.status === 'ready' && transcript.available;
  useEffect(() => {
    const node = scrollRef.current;
    if (!shown || !node || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(() => {
      if (Date.now() - clicked.current < CLICK_GRACE_MS) updateAtBottom();
      else if (atBottom.current) toEnd();
    });
    observer.observe(node);
    for (const child of Array.from(node.children)) observer.observe(child);
    return () => observer.disconnect();
  }, [shown, entries.length === 0]);

  function updateAtBottom() {
    const node = scrollRef.current;
    if (!node) return;
    const bottom = node.scrollHeight - node.scrollTop - node.clientHeight <= BOTTOM_SLACK;
    atBottom.current = bottom;
    if (bottom) setUnseen(0);
  }

  const jump = () => {
    atBottom.current = true;
    setUnseen(0);
    toEnd(true);
  };

  if (transcript.status === 'loading') return <div className="tx-scroll"><LoadingState text="Verlauf wird geladen …" /></div>;
  if (transcript.status === 'error') return <div className="tx-scroll"><ErrorState message="Verlauf konnte nicht geladen werden" detail={transcript.message ?? undefined} onRetry={onRetry} /></div>;
  if (!transcript.available) {
    return (
      <div className="tx-scroll">
        <EmptyState title="Kein Transcript gefunden">
          Gelöscht oder auf einem anderen Rechner. Die Tabs Lauf und Änderungen funktionieren trotzdem.
        </EmptyState>
      </div>
    );
  }

  return (
    <div className="tx-wrap">
      <div ref={scrollRef} className="tx-scroll" onScroll={updateAtBottom} onClickCapture={() => (clicked.current = Date.now())} tabIndex={0} aria-label="Verlauf der Session">
        {entries.length === 0 ? (
          <EmptyState title="Noch keine Einträge">Sobald die Session etwas schreibt, erscheint es hier.</EmptyState>
        ) : (
          <ol className="transcript">
            {entries.map((entry) => (
              <TranscriptEntryView key={entry.id} entry={entry} />
            ))}
          </ol>
        )}
        {!transcript.complete && (
          <p className="tx-footnote" role="status">
            Weitere Einträge werden geladen …
          </p>
        )}
        {transcript.skipped > 0 && (
          <p className="tx-footnote">
            {transcript.skipped === 1 ? 'Eine Zeile' : `${formatNumber(transcript.skipped)} Zeilen`} des Transcripts konnte{transcript.skipped === 1 ? '' : 'n'} nicht gelesen werden.
          </p>
        )}
        {running && transcript.complete && (
          <p className="tx-live">
            <span className="tx-live-dot" aria-hidden="true" />
            Session läuft – neue Einträge erscheinen automatisch.
          </p>
        )}
      </div>
      {unseen > 0 && (
        <button type="button" className="tx-new" onClick={jump}>
          Neue Einträge ↓
        </button>
      )}
    </div>
  );
}
