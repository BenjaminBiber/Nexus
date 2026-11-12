import { useCallback, useEffect, useRef, useState } from 'react';
import { useApi } from '../../api/ApiContext.js';
import type { DispatchRunLogDto, DispatchRunLogPartsDto } from '../../api/dispatchTypes.js';
import { AgentRun } from './AgentRun.js';

type LogState = DispatchRunLogPartsDto | { error: string };

/** So lange (ms) nach dem beobachteten Ende eines Laufs wird höchstens auf sein Ergebnis gewartet: Der Dispatcher trägt
 *  es erst mit seinem nächsten Durchlauf ein (bis zu einer Minute später), dazu etwas Luft */
const AWAIT_RESULT_MS = 90_000;

// Läufe des Dispatchers zu einem Ticket oder einer Frage, je Lauf aufklappbar: im Kopf Ergebnis, Dauer und Modell,
// aufgeklappt der Verlauf (siehe AgentRun und AgentRunBody). Der neueste Lauf ist offen. Solange ein Lauf läuft, lädt die Liste alle
// `intervalMs` nach, dazu die Protokolle offener laufender (und gerade beendeter) Läufe; geöffnete Läufe bleiben offen.
// Endet ein Lauf in dieser Ansicht, lädt die Liste weiter nach, bis sein Ergebnis da ist (höchstens `AWAIT_RESULT_MS`):
// „läuft nicht mehr“ meldet der Server sofort, Ergebnis und Dauer erst nach der Auswertung durch den Dispatcher.
// Der Zeitgeber endet, sobald nichts mehr läuft und auf kein Ergebnis mehr gewartet wird, oder die Ansicht geschlossen
// wird. `onRunning` meldet, ob etwas läuft (das Warten auf ein Ergebnis zählt nicht dazu); ein geänderter Wert von
// `refresh` lädt sofort nach (ein neu hinzugekommener neuester Lauf klappt auf).
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
  const current = useRef(id);
  const runsRef = useRef<DispatchRunLogDto[] | null>(null);
  const openRef = useRef(open);
  openRef.current = open;
  const requested = useRef(new Set<string>());
  // Laufende Nummer der Anfragen je Protokoll; eine ältere Antwort überschreibt keine neuere
  const seq = useRef(new Map<string, number>());
  // Läufe, die in dieser Ansicht geendet haben und deren Ergebnis noch fehlt: Name → Zeitpunkt des beobachteten Endes
  const awaiting = useRef(new Map<string, number>());
  // Wartet noch ein beendeter Lauf auf sein Ergebnis? Dann lädt die Liste weiter nach
  const [waiting, setWaiting] = useState(false);
  const report = useRef(onRunning);
  report.current = onRunning;

  // Anderes Ticket: alles zurücksetzen (vor dem Laden unten)
  useEffect(() => {
    current.current = id;
    runsRef.current = null;
    requested.current = new Set();
    seq.current = new Map();
    awaiting.current = new Map();
    setWaiting(false);
    setRuns(null);
    setError(null);
    setLogs({});
    setOpen(new Set());
  }, [id]);

  // Lädt das Protokoll eines Laufs (Kopf, Arbeitsanweisung, Ausgabe)
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
    // Beendet, aber noch ohne Ergebnis: weiter warten, bis der Dispatcher es eingetragen hat oder die Frist um ist
    const now = Date.now();
    const pending = new Map<string, number>();
    const settled = new Set<string>();
    for (const r of list) {
      const since = awaiting.current.get(r.name) ?? (before.get(r.name) ? now : undefined);
      if (since === undefined || r.running) continue;
      if (r.outcome) settled.add(r.name);
      else if (now - since < AWAIT_RESULT_MS) pending.set(r.name, since);
    }
    awaiting.current = pending;
    setWaiting(pending.size > 0);
    for (const r of list) {
      // Das Protokoll ändert sich, solange der Lauf läuft, bei seinem Ende und wenn sein Ergebnis eintrifft
      const changing = !before.has(r.name) || r.running || before.get(r.name) || settled.has(r.name);
      if (opened.has(r.name) && changing) void loadLog(r.name);
      // Zugeklappt, während es lief: beim nächsten Aufklappen neu laden statt den alten Stand zu zeigen
      else if (changing) requested.current.delete(r.name);
    }
  }, [api, id, loadLog]);

  // Erstes Laden, dazu bei jedem neuen `refresh` (z. B. Ticketstand: ein neuer Lauf kann begonnen haben)
  useEffect(() => {
    void reload();
  }, [reload, refresh]);

  // Nachladen, solange etwas läuft oder ein beendeter Lauf auf sein Ergebnis wartet
  const polling = (runs?.some((r) => r.running) ?? false) || waiting;
  useEffect(() => {
    if (!polling) return undefined;
    const timer = setInterval(() => void reload(), intervalMs);
    return () => clearInterval(timer);
  }, [polling, reload, intervalMs]);

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
      {runs.map((r) => (
        <AgentRun key={r.name} run={r} log={logs[r.name]} open={open.has(r.name)} intervalMs={intervalMs} onToggle={(isOpen) => toggle(r.name, isOpen)} />
      ))}
    </div>
  );
}
