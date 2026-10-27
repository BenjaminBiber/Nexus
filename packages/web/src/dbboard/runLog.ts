/** Farbe des Kreises einer Zeile in der kompakten Ansicht */
export type RunLogTone = 'ok' | 'error' | 'running';

/** Zeile der kompakten Ansicht */
export type RunLogRow =
  /** Schritt mit Kreis: Werkzeugaufruf, Start, Ende; `note` z. B. Testzählung, `error` die Fehlermeldung */
  | { type: 'step'; time: string; text: string; tone: RunLogTone; note?: string; error?: string }
  /** Text von Claude (ohne Kreis) */
  | { type: 'say'; time: string; text: string }
  /** Sonstige Zeile mit Uhrzeit (Meldungen des Dispatchers, ältere Protokolle) */
  | { type: 'line'; time: string; text: string }
  /** Zusammenhängende Ausgabe ohne Uhrzeit (z. B. Einrichtung, stderr) */
  | { type: 'output'; lines: string[] };

const TAGGED = /^(\d\d:\d\d:\d\d) {2}\[(start|say|read|edit|search|test|run|todo|tool|ok|fail|done|abort)(?:#([^\]]+))?\] ?([^\t]*)(?:\t(".*"))?$/;
const TIMED = /^(\d\d:\d\d:\d\d) {2}(.*)$/;
const DISPATCH = /^\[dispatch\] (?:(\d{4}-\d\d-\d\dT[\d:.]+Z) )?(.*)$/;

interface Tagged {
  time: string;
  kind: string;
  ref?: string;
  text: string;
  detail?: string;
}

// Zerlegt eine getaggte Zeile („HH:MM:SS  [art#ref] text<TAB>"detail"“, siehe StreamEventFormatter im Core); sonst null
function tagged(line: string): Tagged | null {
  const m = TAGGED.exec(line);
  if (!m) return null;
  let detail: string | undefined;
  if (m[5]) {
    try {
      detail = JSON.parse(m[5]) as string;
    } catch {
      detail = m[5];
    }
  }
  return { time: m[1]!, kind: m[2]!, ...(m[3] ? { ref: m[3] } : {}), text: m[4]!, ...(detail !== undefined ? { detail } : {}) };
}

// Leerraum zusammengezogen (wie die Kurzfassung im Core)
function collapse(text: string): string {
  return text.replace(/\s+/g, ' ').trim();
}

// ISO-Zeit als „HH:MM:SS“ in Ortszeit
function clock(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '';
  const p = (n: number) => String(n).padStart(2, '0');
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

// Kompakte Ansicht der Ausgabe: Werkzeugaufrufe mit Kreis (grün fertig, rot Fehler, orange läuft), ihre Ergebnisse
// hängen am Aufruf; Claudes Text ohne Kreis; Ausgabe ohne Uhrzeit zusammengefasst. Ein Aufruf ohne Ergebnis gilt als
// laufend, solange der Lauf läuft, danach als abgebrochen.
export function compactRunLog(body: string, running: boolean): RunLogRow[] {
  const rows: RunLogRow[] = [];
  const byRef = new Map<string, Extract<RunLogRow, { type: 'step' }>>();
  const open = new Set<Extract<RunLogRow, { type: 'step' }>>();
  const output = (line: string) => {
    const last = rows[rows.length - 1];
    if (last?.type === 'output') last.lines.push(line);
    else rows.push({ type: 'output', lines: [line] });
  };
  for (const raw of body.split('\n')) {
    const line = raw.replace(/\r$/, '');
    if (!line.trim()) continue;
    const t = tagged(line);
    if (t) {
      if (t.kind === 'ok' || t.kind === 'fail') {
        const step = t.ref ? byRef.get(t.ref) : undefined;
        if (step) {
          open.delete(step);
          step.tone = t.kind === 'ok' ? 'ok' : 'error';
          if (t.kind === 'fail') step.error = t.detail ?? t.text;
          // Bei Fehlern nur eine eigene Zusammenfassung (z. B. Testzählung), nicht den Anfang der Meldung noch einmal
          const repeats = t.kind === 'fail' && collapse(step.error!).startsWith(t.text.replace(/…$/, ''));
          if (t.text && !repeats) step.note = t.text;
        } else if (t.kind === 'fail') {
          rows.push({ type: 'step', time: t.time, text: 'Fehler', tone: 'error', error: t.detail ?? t.text });
        }
        continue;
      }
      if (t.kind === 'say') {
        rows.push({ type: 'say', time: t.time, text: t.detail ?? t.text });
        continue;
      }
      const tone: RunLogTone = t.kind === 'abort' ? 'error' : t.kind === 'start' || t.kind === 'done' ? 'ok' : 'running';
      const step = { type: 'step' as const, time: t.time, text: t.text, tone };
      rows.push(step);
      if (tone === 'running') {
        open.add(step);
        if (t.ref) byRef.set(t.ref, step);
      }
      continue;
    }
    const d = DISPATCH.exec(line);
    if (d) {
      const time = d[1] ? clock(d[1]) : '';
      const exit = /^Ende: Exit-Code (\S+)(.*)$/.exec(d[2]!);
      if (exit) rows.push({ type: 'step', time, text: d[2]!, tone: exit[1] === '0' && !exit[2] ? 'ok' : 'error' });
      else if (/^(Fehler:|Einrichtung fehlgeschlagen)/.test(d[2]!)) rows.push({ type: 'step', time, text: d[2]!, tone: 'error' });
      else if (time) rows.push({ type: 'line', time, text: d[2]! });
      else output(d[2]!);
      continue;
    }
    // Ältere Protokolle ohne Tag: Claudes Text und das Ende erkennen, der Rest bleibt eine schlichte Zeile
    const timed = TIMED.exec(line);
    if (!timed) output(line);
    else if (timed[2]!.startsWith('Claude: ')) rows.push({ type: 'say', time: timed[1]!, text: timed[2]!.slice('Claude: '.length) });
    else if (/^(fertig|beendet \()/.test(timed[2]!)) rows.push({ type: 'step', time: timed[1]!, text: timed[2]!, tone: timed[2]!.startsWith('fertig') ? 'ok' : 'error' });
    else rows.push({ type: 'line', time: timed[1]!, text: timed[2]! });
  }
  if (!running) for (const step of open) step.tone = 'error';
  return rows;
}

// Arbeitsordner des Laufs aus dem Kopf („… startet in <Ordner> (Umsetzung)“); null, wenn er fehlt
export function runRoot(header: string): string | null {
  for (const line of header.split('\n')) {
    const m = /^\[dispatch\] (?:\S+ )?\S+ startet in (.+?) \([^()]+\)\r?$/.exec(line);
    if (m) return m[1]!;
  }
  return null;
}

// Entfernt den Ordner `root` aus Pfaden in allen Schreibweisen (C:\a\b, C:/a/b, /c/a/b); der Ordner allein wird zu „.“
export function relativeTo(root: string | null): (text: string) => string {
  const norm = root?.replace(/\\/g, '/').replace(/\/+$/, '');
  if (!norm) return (text) => text;
  const esc = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').replace(/\//g, '[\\\\/]');
  const drive = /^([A-Za-z]):\/(.*)$/.exec(norm);
  const head = drive ? `(?:${drive[1]}:[\\\\/]|/${drive[1]}/)${esc(drive[2]!)}` : esc(norm);
  const pattern = new RegExp(`${head}(?![\\w.-])([\\\\/])?`, 'gi');
  return (text) => text.replace(pattern, (_m, sep: string | undefined) => (sep ? '' : '.'));
}

// Genaue Ansicht: jede Zeile mit dem vollständigen Aufruf bzw. der ganzen Meldung; Ergebnisse eingerückt darunter,
// leere Erfolgs-Ergebnisse fallen weg. Zeilen ohne Tag bleiben, wie sie sind. Pfade im Ordner `root` erscheinen relativ.
export function detailedRunLog(body: string, root: string | null = null): string {
  const indent = (text: string, by: string) => text.replace(/\n/g, `\n${by}`);
  const relative = relativeTo(root);
  const out: string[] = [];
  for (const raw of body.split('\n')) {
    // Erst zerlegen, dann kürzen: im Protokoll stehen die Backslashes des Details JSON-maskiert
    const line = raw.replace(/\r$/, '');
    const t = tagged(line);
    if (!t) {
      out.push(relative(line));
      continue;
    }
    t.text = relative(t.text);
    if (t.detail !== undefined) t.detail = relative(t.detail);
    const pad = ' '.repeat(t.time.length + 2);
    switch (t.kind) {
      case 'ok':
        if (t.text) out.push(`${pad}  → ${t.text}`);
        break;
      case 'fail':
        out.push(`${pad}  Fehler: ${indent(t.detail ?? t.text, `${pad}  `)}`);
        break;
      case 'say':
        out.push(`${t.time}  Claude: ${indent(t.detail ?? t.text, pad)}`);
        break;
      default:
        out.push(`${t.time}  ${indent(t.detail ?? t.text, pad)}`);
    }
  }
  return out.join('\n');
}

// Kopf des Dispatchers kompakt: ohne Präfix, Zeitstempel und den langen Aufruf von Claude
export function compactRunHeader(header: string): string[] {
  return header
    .split('\n')
    .map((l) => l.replace(/\r$/, ''))
    .filter((l) => l.trim() && !/^\[dispatch\] \S+ (?:-p|--)/.test(l) && !/ --output-format /.test(l))
    .map((l) => {
      const text = l.replace(DISPATCH, (_m, _iso: string | undefined, rest: string) => rest);
      const start = /^(\S+) startet in (.+?) \((.+)\)$/.exec(text);
      if (start) return `${start[1]} · ${start[3]} · ${start[2]!.replace(/\\/g, '/').split('/').filter(Boolean).pop() ?? start[2]}`;
      return text.replace(/^Profil (\S+): /, 'Profil $1 · ').replace(/, (?![^(]*\))/g, ' · ');
    });
}
