import type { TranscriptEntry } from '../api/sessionDetailTypes.js';
import { DISPATCH, TIMED, tagged } from './runLog.js';

type ToolEntry = Extract<TranscriptEntry, { kind: 'tool' }>;

const NO_OUTPUT = 'Erfolgreich – die Ausgabe steht nicht im Protokoll.';
const OUTPUT_HINT = 'Ausgabe ohne Zeitangabe (Einrichtung des Worktrees oder Fehlerausgabe)';
const TOOL_KINDS = new Set(['read', 'edit', 'search', 'test', 'run', 'todo', 'tool']);

// Macht aus der Uhrzeit einer Zeile („HH:MM:SS“, Ortszeit) einen Zeitpunkt: Datum des Laufstarts;
// springt die Uhrzeit um mehr als eine Minute zurück, zählt der nächste Tag
function clockStamper(started: string | null): (time: string) => string {
  const parsed = started ? new Date(started) : new Date();
  const base = Number.isNaN(parsed.getTime()) ? new Date() : parsed;
  let day = 0;
  let last: number | null = started ? base.getHours() * 3600 + base.getMinutes() * 60 + base.getSeconds() : null;
  return (time) => {
    const [h = 0, m = 0, s = 0] = time.split(':').map(Number);
    const seconds = h * 3600 + m * 60 + s;
    if (last !== null && seconds < last - 60) day += 1;
    last = seconds;
    return new Date(base.getFullYear(), base.getMonth(), base.getDate() + day, h, m, s).toISOString();
  };
}

// Name des Tools zu einer Marke des Protokolls (siehe StreamEventFormatter im Core)
function toolName(kind: string, text: string, detail: string | undefined): string {
  switch (kind) {
    case 'read':
      return 'Read';
    case 'edit':
      return text.startsWith('schreibt') ? 'Write' : 'Edit';
    case 'search':
      return text.startsWith('sucht Dateien') ? 'Glob' : 'Grep';
    case 'test':
    case 'run':
      return detail?.startsWith('PS> ') ? 'PowerShell' : 'Bash';
    case 'todo':
      return 'TodoWrite';
    default:
      return (detail ?? text).trim().split(/\s+/)[0] || 'Tool';
  }
}

// Kurzinfo neben dem Tool-Namen: bei Lesen und Ändern nur die Datei
function toolSummary(kind: string, text: string): string {
  return kind === 'read' || kind === 'edit' ? text.replace(/^(liest|ändert|schreibt) /, '') : text;
}

// Eingabe des Aufrufs: der Befehl einer Shell als Objekt, sonst das Detail als Text
function toolInput(detail: string | undefined): unknown {
  if (detail === undefined) return null;
  const shell = /^(?:\$|PS>) ([\s\S]*)$/.exec(detail);
  return shell ? { command: shell[1] } : detail;
}

// Stufe einer Meldung des Dispatchers
function dispatchLevel(text: string): 'info' | 'error' {
  if (/^(Fehler:|Einrichtung fehlgeschlagen)/.test(text)) return 'error';
  const end = /^Ende: Exit-Code (\S+)(.*)$/.exec(text);
  return end && (end[1] !== '0' || end[2]!.trim() !== '') ? 'error' : 'info';
}

// Übersetzt den Text eines Lauf-Protokolls in Einträge des Verlaufs – dieselbe Form, die das Transcript liefert.
// `started` ist der Start des Laufs; er gibt den Uhrzeiten des Protokolls ihr Datum.
export function runLogEntries(body: string, started: string | null): TranscriptEntry[] {
  const entries: TranscriptEntry[] = [];
  const calls = new Map<string, ToolEntry>();
  const stamp = clockStamper(started);
  let lastAt = started ?? new Date().toISOString();
  // Offener Block aufeinanderfolgender Zeilen ohne Uhrzeit
  let output: { entry: ToolEntry; lines: string[] } | null = null;

  body.split('\n').forEach((raw, index) => {
    const line = raw.replace(/\r$/, '');
    if (line.trim() === '' || (index === 0 && line.startsWith('…'))) return;
    const id = `log-${index}`;

    const t = tagged(line);
    if (t) {
      output = null;
      const at = (lastAt = stamp(t.time));
      if (t.kind === 'say') {
        entries.push({ kind: 'text', id, at, text: t.detail ?? t.text });
      } else if (TOOL_KINDS.has(t.kind)) {
        const call: ToolEntry = {
          kind: 'tool', id: t.ref ?? id, at, name: toolName(t.kind, t.text, t.detail),
          summary: toolSummary(t.kind, t.text), input: toolInput(t.detail), result: null,
        };
        if (t.ref) calls.set(t.ref, call);
        entries.push(call);
      } else if (t.kind === 'ok') {
        const call = t.ref ? calls.get(t.ref) : undefined;
        if (call) call.result = { text: t.text.trim() || NO_OUTPUT, isError: false, truncated: false };
      } else if (t.kind === 'fail') {
        const call = t.ref ? calls.get(t.ref) : undefined;
        const text = t.detail ?? t.text;
        if (call) call.result = { text, isError: true, truncated: false };
        else entries.push({ kind: 'system', id, at, text, level: 'error' });
      } else {
        // start, done, abort
        entries.push({ kind: 'system', id, at, text: t.text, level: t.kind === 'abort' ? 'error' : 'info' });
      }
      return;
    }

    const dispatch = DISPATCH.exec(line);
    if (dispatch) {
      output = null;
      if (dispatch[1]) lastAt = dispatch[1];
      entries.push({ kind: 'system', id, at: lastAt, text: dispatch[2]!, level: dispatchLevel(dispatch[2]!) });
      return;
    }

    const timed = TIMED.exec(line);
    if (timed) {
      output = null;
      const at = (lastAt = stamp(timed[1]!));
      const text = timed[2]!;
      if (text.startsWith('Claude: ')) entries.push({ kind: 'text', id, at, text: text.slice('Claude: '.length) });
      else entries.push({ kind: 'system', id, at, text, level: 'info' });
      return;
    }

    // Zeile ohne Uhrzeit: gehört zum offenen Ausgabe-Block oder beginnt einen neuen
    const open = output as { entry: ToolEntry; lines: string[] } | null;
    if (open) {
      open.lines.push(line);
      open.entry.summary = `${open.lines.length} Zeilen`;
      open.entry.result = { text: open.lines.join('\n'), isError: false, truncated: false };
      return;
    }
    const entry: ToolEntry = {
      kind: 'tool', id, at: lastAt, name: 'Ausgabe', summary: '1 Zeile', input: OUTPUT_HINT,
      result: { text: line, isError: false, truncated: false },
    };
    output = { entry, lines: [line] };
    entries.push(entry);
  });
  return entries;
}
