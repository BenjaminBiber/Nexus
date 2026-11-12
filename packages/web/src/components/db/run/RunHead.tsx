import type { DispatchRunLogDto } from '../../../api/dispatchTypes.js';
import { formatDate } from '../../../dbboard/formatDate.js';
import { formatDuration } from '../../../sessions/format.js';

/** Wie die Art eines Laufs im Titel heißt */
export const KIND_LABELS: Record<DispatchRunLogDto['kind'], string> = { work: 'Lauf', merge: 'Merge-Agent', research: 'Recherche', plan: 'Planung' };

/** Farbe der Marke: gut, Achtung, Fehler, neutral oder laufend */
export type RunTone = 'ok' | 'warn' | 'error' | 'neutral' | 'running';

/** Ergebnis aus der Board-Datenbank → Marke und Ton */
const OUTCOMES: Record<string, { label: string; tone: RunTone }> = {
  done: { label: 'erledigt', tone: 'ok' },
  review: { label: 'Review', tone: 'ok' },
  'merge-session': { label: 'Merge', tone: 'ok' },
  research: { label: 'Recherche', tone: 'ok' },
  human: { label: 'Abnahme', tone: 'warn' },
  question: { label: 'Frage', tone: 'warn' },
  blocked: { label: 'blockiert', tone: 'warn' },
  'pause-limit': { label: 'Limit', tone: 'warn' },
  taken: { label: 'übernommen', tone: 'warn' },
  gone: { label: 'Ticket weg', tone: 'warn' },
  abort: { label: 'Abbruch', tone: 'error' },
  'abort-final': { label: 'Abbruch', tone: 'error' },
  'setup-failed': { label: 'Einrichtung gescheitert', tone: 'error' },
  'pause-claude-missing': { label: 'Claude fehlt', tone: 'error' },
};

// Marke eines Laufs: „läuft“, solange er läuft; danach sein Ergebnis, ohne bekanntes Ergebnis „beendet“
export function runOutcome(run: DispatchRunLogDto): { label: string; tone: RunTone } {
  if (run.running) return { label: 'läuft', tone: 'running' };
  return (run.outcome ? OUTCOMES[run.outcome] : undefined) ?? { label: 'beendet', tone: 'neutral' };
}

// Kopfzeile eines Laufs (Inhalt des <summary>): Marke mit Ergebnis, Titel, gedämpft Dauer und Modell
export function RunHead({ run }: { run: DispatchRunLogDto }) {
  const outcome = runOutcome(run);
  const facts = [
    typeof run.durationMs === 'number' ? formatDuration(run.durationMs) : null,
    run.model ? run.model.replace(/^claude-/, '') : null,
  ].filter((fact): fact is string => fact !== null);
  return (
    <>
      <span className={outcome.tone === 'running' ? 'badge running' : `badge outcome-${outcome.tone}`}>{outcome.label}</span>
      <span className="run-title">{`${KIND_LABELS[run.kind]} vom ${run.started ? formatDate(run.started) : run.name}`}</span>
      {facts.length > 0 && <span className="muted run-facts">{facts.join(' · ')}</span>}
    </>
  );
}

// Abbruchgrund eines beendeten Laufs als eine Zeile unter dem Kopf; ohne Grund nichts
export function RunAbortReason({ run }: { run: DispatchRunLogDto }) {
  if (run.running || !run.abortReason) return null;
  return <p className="run-abort-reason">{run.abortReason}</p>;
}
