import type { TranscriptEntry } from '../api/sessionDetailTypes.js';
import { DISPATCH, runHeaderLines, TIMED, tagged } from './runLog.js';

/** Hook-Fehler am Ende einer Sitzung, z. B. „SessionEnd-Hook fehlgeschlagen“ mit Befehl und Meldung */
export interface RunHookFailure {
  title: string;
  detail: string;
}

/** Abschluss eines Laufs laut Protokoll; bei einem laufenden Lauf ist alles leer */
export interface RunSessionEnd {
  /** „fertig“ oder „beendet (error_max_turns)“; ok: Claude meldete Erfolg */
  status: { label: string; ok: boolean } | null;
  /** Weitere Angaben der Ergebniszeile, z. B. „27 Schritte“, „11 min“ */
  facts: string[];
  /** Exit-Code von Claude, z. B. „Exit-Code 0“; ok: 0 ohne Zeitlimit */
  exit: { label: string; ok: boolean } | null;
  hooks: RunHookFailure[];
}

/** Rohzeile von Claude: „SessionEnd hook [Befehl] failed: Meldung“ */
const HOOK_FAILED = /^(\S+) hook \[(.+?)\] failed: (.*)$/;
/** Hooks, die erst nach der Ergebniszeile laufen */
const END_HOOKS = /^(?:SessionEnd|Stop)$/;
/** Letzte Zeile des Dispatchers: „Ende: Exit-Code 0“, wahlweise „, Zeitlimit“ */
const END = /^Ende: Exit-Code ([^\s,]+)(.*)$/;
/** Ergebniszeile (Marken done und abort): „fertig, 27 Schritte, 11 min“, „beendet (error_max_turns), …“ */
const RESULT = /^(?:fertig|beendet \([^)]*\))(?:, \d+ Schritte)?(?:, \d+ min)?$/;
/** Startzeile (Marke start) */
const START = /^Claude startet \(.+\)$/;
/** Grenzen eines Profils: „150 Schritte“, „90 min“ */
const LIMIT = /^\d+ (?:Schritte|min)$/;

// Angaben der Profilzeile „Profil standard: Modell opus, Effort low, 150 Schritte, 90 min (Label profil:standard)“
// einzeln: Profil, Modell, Effort, Limit und je Teil der Grund der Wahl (ohne „Label “); keine Profilzeile: null
function profileChips(line: string): string[] | null {
  const m = /^Profil (\S+): (.*)$/.exec(line);
  if (!m) return null;
  let rest = m[2]!;
  let why: string[] = [];
  const reason = /^(.*) \(([^()]*)\)$/.exec(rest);
  if (reason) {
    rest = reason[1]!;
    why = reason[2]!.split(', ').map((part) => part.replace(/^Label /, ''));
  }
  const parts = rest.split(', ');
  const limits = parts.filter((part) => LIMIT.test(part));
  const settings = parts.filter((part) => !LIMIT.test(part));
  return [`Profil ${m[1]}`, ...settings, ...(limits.length > 0 ? [`Limit ${limits.join(' · ')}`] : []), ...why].filter((chip) => chip.trim());
}

// Einstellungen eines Laufs aus dem Kopf des Dispatchers als kurze Angaben (Profil, Modell, Effort, Limit, Grund);
// andere Zeilen des Kopfs bleiben je eine Angabe
export function runChips(header: string): string[] {
  return runHeaderLines(header).flatMap((line) => profileChips(line) ?? [line]);
}

// Erste Zeile des Verlaufs: „Claude startet (Modell)“ aus dem Protokoll; fehlt sie dort (nur das Ende ist geladen),
// aus dem Modell des Laufs; ohne beides null
export function runStartLine(body: string, model: string | null): string | null {
  for (const line of body.split('\n')) {
    const clean = line.replace(/\r$/, '');
    const t = tagged(clean);
    if (t?.kind === 'start') return t.text;
    // Ältere Protokolle ohne Marken: „12:00:01  Claude startet (opus)“
    const plain = t ? null : TIMED.exec(clean)?.[2];
    if (plain && START.test(plain)) return plain;
  }
  return model ? `Claude startet (${model})` : null;
}

// Liest das Ende eines Laufs aus dem Protokoll: Ergebniszeile von Claude, Exit-Code laut Dispatcher und die Hook-Fehler
// am Sitzungsende (nach der Ergebniszeile, dazu SessionEnd und Stop); gleiche Hook-Fehler stehen einmal da
export function runSessionEnd(body: string): RunSessionEnd {
  const end: RunSessionEnd = { status: null, facts: [], exit: null, hooks: [] };
  const seen = new Set<string>();
  let closed = false;
  for (const raw of body.split('\n')) {
    const line = raw.replace(/\r$/, '');
    const t = tagged(line);
    if (t) {
      if (t.kind === 'done' || t.kind === 'abort') {
        const [label = '', ...facts] = t.text.split(', ');
        end.status = { label, ok: t.kind === 'done' };
        end.facts = facts;
        closed = true;
      }
      continue;
    }
    const dispatch = DISPATCH.exec(line);
    if (dispatch) {
      const exit = END.exec(dispatch[2]!);
      if (exit) {
        const extra = exit[2]!.replace(/^,\s*/, '').trim();
        end.exit = { label: `Exit-Code ${exit[1]}${extra ? ` · ${extra}` : ''}`, ok: exit[1] === '0' && !extra };
      }
      continue;
    }
    const hook = HOOK_FAILED.exec(TIMED.exec(line)?.[2] ?? '');
    if (!hook || !(closed || END_HOOKS.test(hook[1]!))) continue;
    const failure = { title: `${hook[1]}-Hook fehlgeschlagen`, detail: `[${hook[2]}] ${hook[3]}` };
    const key = `${failure.title}\n${failure.detail}`;
    if (seen.has(key)) continue;
    seen.add(key);
    end.hooks.push(failure);
  }
  return end;
}

// Rahmen eines Laufs, der nicht im Verlauf steht, sondern darüber (Start) und darunter (Ende, Hook-Fehler am
// Sitzungsende): Systemeinträge, wie sie das Lauf-Protokoll liefert
export function isRunFrameEntry(entry: TranscriptEntry): boolean {
  if (entry.kind !== 'system') return false;
  const hook = HOOK_FAILED.exec(entry.text);
  return START.test(entry.text) || RESULT.test(entry.text) || END.test(entry.text) || (hook !== null && END_HOOKS.test(hook[1]!));
}

// Teilt einen beendeten Lauf in Verlauf und Ergebnis: Endet er (vor Systemeinträgen und Nachdenken) mit einem Text,
// ist das die Antwort des Agenten; sie steht dann nicht mehr im Verlauf. Läuft er noch, gibt es kein Ergebnis.
export function splitRunResult(entries: TranscriptEntry[], running: boolean): { entries: TranscriptEntry[]; result: string | null } {
  if (running) return { entries, result: null };
  for (let i = entries.length - 1; i >= 0; i -= 1) {
    const entry = entries[i]!;
    if (entry.kind === 'system' || entry.kind === 'thinking') continue;
    if (entry.kind === 'text' && entry.text.trim() !== '') {
      return { entries: [...entries.slice(0, i), ...entries.slice(i + 1)], result: entry.text };
    }
    break;
  }
  return { entries, result: null };
}
