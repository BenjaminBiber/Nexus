import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useApi } from '../../api/ApiContext.js';
import type { DispatchRunLogDto, DispatchRunLogPartsDto } from '../../api/dispatchTypes.js';
import { formatDate } from '../../dbboard/formatDate.js';
import { compactRunHeader, compactRunLog, detailedRunLog, runRoot } from '../../dbboard/runLog.js';

const KIND_LABELS: Record<DispatchRunLogDto['kind'], string> = { work: 'Lauf', merge: 'Merge-Agent', research: 'Recherche', plan: 'Planung' };
/** Näher als so viele Pixel am Ende gilt ein Protokoll als „unten“ und bleibt beim Nachladen am Ende */
const STICK_PX = 40;
/** Gewählte Ansicht der Protokolle (je Browser) */
const VIEW_KEY = 'harness.agentRuns.view';
const TONE_LABELS = { ok: 'erfolgreich', error: 'Fehler', running: 'in Arbeit' } as const;

type LogView = 'compact' | 'detailed';

function storedView(): LogView {
  try {
    return localStorage.getItem(VIEW_KEY) === 'detailed' ? 'detailed' : 'compact';
  } catch {
    return 'compact';
  }
}

type LogState = DispatchRunLogPartsDto | { error: string };

// Läufe des Dispatchers zu einem Ticket oder einer Frage, je Lauf aufklappbar mit Protokoll und getrennter Arbeitsanweisung.
// Das Protokoll gibt es kompakt (lesbare Schritte mit farbigem Kreis) oder detailliert (genaue Befehle), umschaltbar.
// Der neueste Lauf ist offen. Solange ein Lauf läuft, lädt die Liste alle `intervalMs` nach, dazu die Protokolle offener
// laufender (und gerade beendeter) Läufe; geöffnete Läufe und Scrollstand bleiben, ein Protokoll am Ende bleibt am Ende.
// Der Zeitgeber endet, sobald nichts mehr läuft oder die Ansicht geschlossen wird. `onRunning` meldet, ob etwas läuft;
// ein geänderter Wert von `refresh` lädt sofort nach (ein neu hinzugekommener neuester Lauf klappt auf).
export function AgentRuns({ id, empty = 'Zu diesem Ticket gab es noch keinen Agent-Lauf.', intervalMs = 3000, refresh, onRunning }: {
  id: string;
  empty?: string;
  intervalMs?: number;
  refresh?: string | number;
  onRunning?: (running: boolean) => void;
}) {
  const api = useApi();
  const [runs, setRuns] = useState<DispatchRunLogDto[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [logs, setLogs] = useState<Record<string, LogState>>({});
  const [open, setOpen] = useState<Set<string>>(() => new Set());
  const [view, setView] = useState<LogView>(storedView);
  const current = useRef(id);
  const runsRef = useRef<DispatchRunLogDto[] | null>(null);
  const openRef = useRef(open);
  openRef.current = open;
  const requested = useRef(new Set<string>());
  // Laufende Nummer der Anfragen je Protokoll; eine ältere Antwort überschreibt keine neuere
  const seq = useRef(new Map<string, number>());
  const pres = useRef(new Map<string, HTMLElement>());
  const stick = useRef(new Set<string>());
  const report = useRef(onRunning);
  report.current = onRunning;

  // Anderes Ticket: alles zurücksetzen (vor dem Laden unten)
  useEffect(() => {
    current.current = id;
    runsRef.current = null;
    requested.current = new Set();
    seq.current = new Map();
    setRuns(null);
    setError(null);
    setLogs({});
    setOpen(new Set());
  }, [id]);

  // Lädt ein Protokoll; vorher merken, ob es am Ende stand (dann nach dem Zeichnen wieder ans Ende)
  const loadLog = useCallback(async (name: string) => {
    requested.current.add(name);
    const mine = (seq.current.get(name) ?? 0) + 1;
    seq.current.set(name, mine);
    let next: LogState;
    try {
      next = await api.getDispatchRunLog(name);
    } catch (e) {
      next = { error: (e as Error).message };
    }
    if (current.current !== id || seq.current.get(name) !== mine) return;
    const pre = pres.current.get(name);
    if (!pre || pre.scrollHeight - pre.scrollTop - pre.clientHeight < STICK_PX) stick.current.add(name);
    setLogs((all) => ({ ...all, [name]: next }));
  }, [api, id]);

  // Lädt die Liste; beim ersten Mal den neuesten Lauf öffnen, danach offene laufende und gerade beendete nachladen
  const reload = useCallback(async () => {
    let list: DispatchRunLogDto[];
    try {
      list = await api.getDispatchRuns(id);
    } catch (e) {
      if (current.current === id) setError((e as Error).message);
      return;
    }
    if (current.current !== id) return;
    const first = runsRef.current === null;
    const before = new Map((runsRef.current ?? []).map((r) => [r.name, r.running]));
    // Neuester Lauf beim ersten Laden oder neu hinzugekommen: aufklappen
    const newest = list[0]?.name;
    const fresh = newest !== undefined && (first || !before.has(newest));
    const opened = fresh ? new Set([...(first ? [] : openRef.current), newest]) : openRef.current;
    if (fresh) setOpen(opened);
    runsRef.current = list;
    setRuns(list);
    setError(null);
    report.current?.(list.some((r) => r.running));
    for (const r of list) {
      const changing = !before.has(r.name) || r.running || before.get(r.name);
      if (opened.has(r.name) && changing) void loadLog(r.name);
      // Zugeklappt, während es lief: beim nächsten Aufklappen neu laden statt den alten Stand zu zeigen
      else if (changing) requested.current.delete(r.name);
    }
  }, [api, id, loadLog]);

  // Erstes Laden, dazu bei jedem neuen `refresh` (z. B. Ticketstand: ein neuer Lauf kann begonnen haben)
  useEffect(() => {
    void reload();
  }, [reload, refresh]);

  const running = runs?.some((r) => r.running) ?? false;
  useEffect(() => {
    if (!running) return undefined;
    const timer = setInterval(() => void reload(), intervalMs);
    return () => clearInterval(timer);
  }, [running, reload, intervalMs]);

  // Protokolle, die am Ende standen, nach dem Zeichnen wieder ans Ende
  useLayoutEffect(() => {
    for (const name of stick.current) {
      const pre = pres.current.get(name);
      if (pre) pre.scrollTop = pre.scrollHeight;
    }
    stick.current.clear();
  }, [logs]);

  const chooseView = (next: LogView) => {
    setView(next);
    try {
      localStorage.setItem(VIEW_KEY, next);
    } catch {
      // ohne Speicher gilt die Wahl nur bis zum Neuladen
    }
  };
  const logRef = (name: string) => (el: HTMLElement | null) => {
    if (el) pres.current.set(name, el);
    else pres.current.delete(name);
  };

  // Auf- oder zugeklappt (Nutzer oder Browser); beim ersten Öffnen das Protokoll laden
  const toggle = (name: string, isOpen: boolean) => {
    setOpen((prev) => {
      if (prev.has(name) === isOpen) return prev;
      const next = new Set(prev);
      if (isOpen) next.add(name);
      else next.delete(name);
      return next;
    });
    if (isOpen && !requested.current.has(name)) void loadLog(name);
  };

  if (!runs) return error ? <p role="alert" className="field-error">{error}</p> : <p className="muted">Wird geladen …</p>;
  if (!runs.length) return <p className="muted">{empty}</p>;
  return (
    <div className="agent-runs">
      {error && <p role="alert" className="field-error">{error}</p>}
      {runs.map((r) => {
        const log = logs[r.name];
        return (
          <details key={r.name} className="agent-run" open={open.has(r.name)} onToggle={(e) => toggle(r.name, e.currentTarget.open)}>
            <summary>
              <span className={r.running ? 'badge running' : 'badge'}>{r.running ? 'läuft' : 'beendet'}</span>
              <span>{KIND_LABELS[r.kind]} vom {r.started ? formatDate(r.started) : r.name}</span>
              {/* Umschalter gilt für alle Läufe; preventDefault, damit der Klick den Lauf nicht zuklappt */}
              {open.has(r.name) && (
                <span className="log-view-toggle" role="group" aria-label="Ansicht des Protokolls">
                  {(['compact', 'detailed'] as const).map((v) => (
                    <button key={v} type="button" aria-pressed={view === v} onClick={(e) => { e.preventDefault(); chooseView(v); }}>
                      {v === 'compact' ? 'Kompakt' : 'Detailliert'}
                    </button>
                  ))}
                </span>
              )}
            </summary>
            {!log && open.has(r.name) && <p className="muted">Wird geladen …</p>}
            {log && 'error' in log && <p role="alert" className="field-error">{log.error}</p>}
            {log && !('error' in log) && (
              <>
                {log.header && view === 'detailed' && <pre className="agent-run-head">{log.header}</pre>}
                {log.header && view === 'compact' && (
                  <div className="agent-run-head compact">{compactRunHeader(log.header).map((l, i) => <div key={i}>{l}</div>)}</div>
                )}
                {log.prompt === null && log.body.startsWith('…') && <p className="muted">Nur das Ende des Protokolls – es ist sehr lang.</p>}
                {!log.body ? (
                  <pre className="dispatch-log" ref={logRef(r.name)}>{r.running ? 'Noch keine Ausgabe …' : '(keine Ausgabe)'}</pre>
                ) : view === 'detailed' ? (
                  <pre className="dispatch-log" ref={logRef(r.name)}>{detailedRunLog(log.body, runRoot(log.header))}</pre>
                ) : (
                  <CompactLog body={log.body} running={r.running} logRef={logRef(r.name)} />
                )}
                {log.prompt !== null && (
                  <details className="agent-prompt">
                    <summary className="muted">Arbeitsanweisung an den Agent</summary>
                    <pre className="dispatch-log">{log.prompt}</pre>
                  </details>
                )}
              </>
            )}
          </details>
        );
      })}
    </div>
  );
}

// Kompakte Ansicht eines Protokolls: Schritte mit Kreis (grün fertig, rot Fehler, orange in Arbeit), Claudes Text dazwischen
function CompactLog({ body, running, logRef }: { body: string; running: boolean; logRef: (el: HTMLElement | null) => void }) {
  const rows = compactRunLog(body, running);
  return (
    <ol className="dispatch-log run-steps" ref={logRef}>
      {rows.map((row, i) => {
        if (row.type === 'output') {
          return (
            <li key={i} className="run-output">
              <details>
                <summary className="muted">Ausgabe ({row.lines.length} {row.lines.length === 1 ? 'Zeile' : 'Zeilen'})</summary>
                <pre>{row.lines.join('\n')}</pre>
              </details>
            </li>
          );
        }
        if (row.type === 'say' || row.type === 'line') {
          return (
            <li key={i} className={row.type === 'say' ? 'run-say' : 'run-line'}>
              <span className="run-time">{row.time}</span>
              <span className="run-text">{row.text}</span>
            </li>
          );
        }
        return (
          <li key={i} className="run-step">
            <span className="run-time">{row.time}</span>
            <span className={`run-dot ${row.tone}`} role="img" aria-label={TONE_LABELS[row.tone]} title={TONE_LABELS[row.tone]} />
            <span className="run-text">
              {row.text}
              {row.note && <span className="run-note"> · {row.note}</span>}
              {row.error && <span className="run-error">{row.error}</span>}
            </span>
          </li>
        );
      })}
    </ol>
  );
}
