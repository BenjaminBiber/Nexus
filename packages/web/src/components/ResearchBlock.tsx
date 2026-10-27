import type { DispatchRunDto } from '../api/dispatchTypes.js';
import type { ResearchDto } from '../api/questionTypes.js';
import { MarkdownView } from '../markdown/MarkdownView.js';
import { CopyCommand } from './CopyCommand.js';
import { OptionCard } from './OptionCard.js';

const KIND: Record<string, string> = { human: 'Mensch', session: 'Session', subagent: 'Subagent' };

// Datum und Uhrzeit, deutsch
function when(iso: string): string {
  return new Date(iso).toLocaleString('de-DE', { day: '2-digit', month: '2-digit', year: 'numeric', hour: '2-digit', minute: '2-digit' });
}

// Uhrzeit HH:MM, deutsch
function hhmm(iso: string): string {
  return new Date(iso).toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' });
}

// Kontext und Optionen einer Recherche; `onPick` macht die Optionen wählbar
function ResearchBody({ research, picked, disabled, onPick }: { research: ResearchDto; picked?: string | null; disabled?: boolean; onPick?: (id: string | null) => void }) {
  return (
    <>
      {research.context.trim() && (
        <div>
          <div className="muted option-label">Kontext</div>
          <MarkdownView source={research.context} />
        </div>
      )}
      {research.options.length > 0 && (
        <div>
          <div className="muted option-label">Optionen</div>
          <div className="option-grid">
            {research.options.map((o) => (
              <OptionCard
                key={o.id}
                option={o}
                picked={picked === o.id}
                disabled={disabled}
                onPick={onPick ? () => onPick(picked === o.id ? null : o.id) : undefined}
              />
            ))}
          </div>
        </div>
      )}
    </>
  );
}

// „Recherche starten“: Claude-Session über den Dispatcher (claude -p im eigenen, losgelösten Worktree)
export interface ResearchStart {
  /** Nur offene Fragen lassen sich recherchieren */
  open: boolean;
  /** Laufende Recherche zu dieser Frage oder null */
  running: DispatchRunDto | null;
  /** Pause des Dispatchers (z. B. Nutzungslimit): der Server lehnt den Start bis dahin ab */
  pausedUntil?: string | null;
  busy: boolean;
  error: string | null;
  onStart: () => void;
  /** Öffnet das Protokoll eines Laufs */
  onShowLog?: (name: string) => void;
}

// Knopf „Recherche starten“ mit Hinweis, warum er gesperrt ist bzw. warum der Start gerade abgelehnt würde (Pause),
// laufender Recherche und Fehler
function ResearchStartButton({ start }: { start: ResearchStart }) {
  const hint = !start.open
    ? 'Nur offene Fragen lassen sich recherchieren.'
    : start.running
      ? 'Für diese Frage läuft schon eine Recherche.'
      : start.pausedUntil
        ? `Der Dispatcher pausiert bis ${hhmm(start.pausedUntil)} (z. B. Nutzungslimit) – eine Recherche startet erst danach.`
        : null;
  return (
    <div className="research-start">
      <div className="actions">
        <button type="button" className="btn primary" disabled={!start.open || start.running !== null || start.busy} onClick={start.onStart}>
          Recherche starten
        </button>
        {hint && <span className="muted">{hint}</span>}
      </div>
      {start.running && (
        <p role="status" className="muted">
          Recherche läuft – Ergebnis erscheint hier automatisch (Protokoll{' '}
          {start.onShowLog ? (
            <button type="button" className="link-btn" onClick={() => start.onShowLog?.(start.running!.log)}>{start.running.log}</button>
          ) : (
            <span className="mono">{start.running.log}</span>
          )}
          ).
        </p>
      )}
      {start.error && (
        <p role="alert" className="field-error">
          {start.error}
        </p>
      )}
    </div>
  );
}

// Recherchen einer Frage: die neueste ausgeklappt, ältere eingeklappt und nur lesbar, dazu „Recherche starten“ und der Befehl für eine Session.
export function ResearchBlock({
  questionId,
  research,
  selectable = false,
  picked = null,
  optionsDisabled = false,
  onPick,
  command,
  start,
}: {
  questionId: string;
  research: ResearchDto[];
  selectable?: boolean;
  picked?: string | null;
  /** Optionen sind gesperrt (Freitext ist eingegeben) */
  optionsDisabled?: boolean;
  onPick?: (id: string | null) => void;
  /** Befehl unter „Recherche anfordern“ (Standard: /ai-harness:board-question <Nummer>) */
  command?: string;
  /** Knopf „Recherche starten“ (ohne: nur der Befehl zum Kopieren) */
  start?: ResearchStart;
}) {
  const current = research.at(-1);
  const earlier = research.slice(0, -1).reverse();
  return (
    <section className="research" aria-labelledby="research-heading">
      <div className="research-head">
        <h2 id="research-heading" className="h3">
          Recherchen
        </h2>
        {research.length > 0 && <span className="muted">{research.length === 1 ? '1 Recherche' : `${research.length} Recherchen, die neueste gilt`}</span>}
      </div>
      {current && (
        <section className="card research-current" aria-label="Aktuelle Recherche">
          <div className="research-meta">
            <strong>Aktuell</strong>
            <span className="badge q-ready">neueste</span>
            <span className="muted">
              {when(current.at)} · {current.author} ({KIND[current.kind] ?? current.kind})
            </span>
          </div>
          <ResearchBody research={current} picked={selectable ? picked : undefined} disabled={optionsDisabled} onPick={selectable ? onPick : undefined} />
        </section>
      )}
      {earlier.map((r) => (
        <details key={r.at} className="research-earlier">
          <summary>
            <span>
              {when(r.at)} · {r.author} ({KIND[r.kind] ?? r.kind})
            </span>
            <span className="badge">ältere Recherche</span>
          </summary>
          <div className="research-earlier-body">
            <ResearchBody research={r} />
          </div>
        </details>
      ))}
      <section className="card research-request" aria-label="Recherche anfordern">
        <h3 className="h3">Recherche anfordern</h3>
        <p className="muted">
          {research.length === 0
            ? 'Noch keine Recherche. Ein Agent kann im Code nachsehen und Optionen vorschlagen.'
            : 'Auch mit vorhandener Recherche möglich: Eine neue Recherche kommt dazu, die frühere bleibt sichtbar.'}
          {start
            ? ' „Recherche starten“ lässt sofort eine Claude-Session (claude -p, nur lesend, eigener Worktree) recherchieren; oder den Befehl in einer eigenen Session ausführen.'
            : ' Befehl in einer Session ausführen.'}
        </p>
        {start && <ResearchStartButton start={start} />}
        <CopyCommand command={command ?? `/ai-harness:board-question ${questionId}`} />
      </section>
    </section>
  );
}
