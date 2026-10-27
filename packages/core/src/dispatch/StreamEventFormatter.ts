import { ShellCommandSummary } from './ShellCommandSummary.js';

type Input = Record<string, unknown>;

interface StreamEvent {
  type?: unknown;
  subtype?: unknown;
  model?: unknown;
  num_turns?: unknown;
  duration_ms?: unknown;
  message?: { content?: unknown };
}

interface ContentBlock {
  type?: unknown;
  id?: unknown;
  tool_use_id?: unknown;
  text?: unknown;
  name?: unknown;
  input?: unknown;
  is_error?: unknown;
  content?: unknown;
}

/**
 * Art einer Protokollzeile: Start, Text von Claude, Werkzeugaufrufe (lesen, ändern, suchen, testen, ausführen, Aufgabenliste,
 * sonstiges), Ergebnis eines Aufrufs (ok/fail, über `ref` zugeordnet), Ende des Laufs (done/abort); raw = Zeile ohne JSON
 */
export type RunLogLineKind = 'start' | 'say' | 'read' | 'edit' | 'search' | 'test' | 'run' | 'todo' | 'tool' | 'ok' | 'fail' | 'done' | 'abort' | 'raw';

/** Eine Zeile des Lauf-Protokolls: Kurzfassung `text`, dazu der genaue Aufruf bzw. die ganze Meldung in `detail` */
export interface RunLogLine {
  kind: RunLogLineKind;
  text: string;
  /** ID des Werkzeugaufrufs (bei Aufrufen und ihren Ergebnissen) */
  ref?: string;
  detail?: string;
}

const DETAIL_MAX = 4000;

// Macht aus der Ausgabe von `claude -p --output-format stream-json` Protokollzeilen (Port aus dem alten Board).
// Jede Zeile hat eine Art, eine lesbare Kurzfassung und den genauen Aufruf; im Protokoll steht sie als
// „HH:MM:SS  [art#ref] text<TAB>"detail"“, die Oberfläche zeigt daraus wahlweise die kompakte oder die genaue Ansicht.
// Das Format legt Claude Code fest: Unbekanntes fällt still heraus, Zeilen ohne JSON bleiben, wie sie sind.
// Eine Instanz hält den Rest einer unvollständigen Zeile und die Art der offenen Aufrufe, daher je Ausgabestrom eine eigene.
export class StreamEventFormatter {
  private pending = '';
  private readonly tools = new Map<string, RunLogLineKind>();
  private readonly shell: ShellCommandSummary;
  private readonly rootPattern: RegExp | null;

  /** `root`: Arbeitsordner des Laufs; Pfade darin erscheinen in der Kurzfassung relativ */
  constructor(options: { root?: string } = {}) {
    this.rootPattern = options.root ? this.pathPattern(options.root) : null;
    this.shell = new ShellCommandSummary((text) => this.relative(text));
  }

  // Nimmt einen Block der Ausgabe und liefert die Einträge aller darin vollständigen Zeilen
  push(chunk: string): RunLogLine[] {
    const parts = (this.pending + chunk).split('\n');
    this.pending = parts.pop() ?? '';
    return parts.flatMap((line) => this.readable(line));
  }

  // Liefert den Rest ohne Zeilenende (am Ende des Laufs)
  flush(): RunLogLine[] {
    const rest = this.pending;
    this.pending = '';
    return this.readable(rest);
  }

  // Ein ganzer Text als lesbare Zeilen (für Auswertungen und Kommentare nach dem Lauf)
  format(text: string): string[] {
    return text.split('\n').flatMap((line) => this.readable(line)).flatMap((e) => this.plain(e));
  }

  // Eine Zeile als lesbare Zeilen; null, wenn sie kein JSON-Objekt ist (dann bleibt sie, wie sie ist)
  describe(line: string): string[] | null {
    return this.entries(line)?.flatMap((e) => this.plain(e)) ?? null;
  }

  // Eine Zeile als Einträge; null, wenn sie kein JSON-Objekt ist
  entries(line: string): RunLogLine[] | null {
    const ev = this.parse(line);
    if (!ev) return null;
    const content = Array.isArray(ev.message?.content) ? (ev.message.content as ContentBlock[]) : [];
    switch (ev.type) {
      case 'system':
        return ev.subtype === 'init' ? [{ kind: 'start', text: `Claude startet (${typeof ev.model === 'string' ? ev.model : 'Modell unbekannt'})` }] : [];
      case 'assistant':
        return content.flatMap((c): RunLogLine[] => {
          if (c?.type === 'text' && String(c.text ?? '').trim()) {
            const text = String(c.text);
            const short = this.shorten(text, 400);
            return [{ kind: 'say', text: short, ...(short.length < text.trim().length ? { detail: this.clip(text.trim()) } : {}) }];
          }
          if (c?.type === 'tool_use') {
            const entry = this.tool(String(c.name ?? '?'), (c.input && typeof c.input === 'object' ? c.input : {}) as Input);
            const ref = typeof c.id === 'string' ? c.id : undefined;
            if (ref) this.tools.set(ref, entry.kind);
            return [{ ...entry, ...(ref ? { ref } : {}) }];
          }
          return [];
        });
      case 'user':
        return content.filter((c) => c?.type === 'tool_result').flatMap((c) => this.result(c));
      case 'result': {
        const turns = typeof ev.num_turns === 'number' ? `, ${ev.num_turns} Schritte` : '';
        const minutes = typeof ev.duration_ms === 'number' ? `, ${Math.round(ev.duration_ms / 60_000)} min` : '';
        const ok = ev.subtype === 'success';
        return [{ kind: ok ? 'done' : 'abort', text: `${ok ? 'fertig' : `beendet (${typeof ev.subtype === 'string' ? ev.subtype : '?'})`}${turns}${minutes}` }];
      }
      default:
        return [];
    }
  }

  // Ist die Zeile ein Ereignis von stream-json (JSON-Objekt mit `type`)?
  isEvent(line: string): boolean {
    return typeof this.parse(line)?.type === 'string';
  }

  // Protokollzeile in Ortszeit: „HH:MM:SS  text“ für Zeilen ohne JSON, sonst „HH:MM:SS  [art#ref] text<TAB>"detail"“
  stamped(entry: RunLogLine | string, at: Date): string {
    const p = (n: number) => String(n).padStart(2, '0');
    const time = `${p(at.getHours())}:${p(at.getMinutes())}:${p(at.getSeconds())}`;
    if (typeof entry === 'string' || entry.kind === 'raw') return `${time}  ${typeof entry === 'string' ? entry : entry.text}\n`;
    const tag = `[${entry.kind}${entry.ref ? `#${entry.ref}` : ''}]`;
    return `${time}  ${tag} ${entry.text}${entry.detail !== undefined ? `\t${JSON.stringify(entry.detail)}` : ''}\n`;
  }

  // Was ein Werkzeugaufruf tut: Kurzfassung und genauer Aufruf
  private tool(name: string, input: Input): RunLogLine {
    const path = typeof input.file_path === 'string' ? input.file_path : typeof input.path === 'string' ? input.path : '';
    const file = this.relative(path);
    switch (name) {
      case 'Read': return { kind: 'read', text: `liest ${file}`, detail: `Read ${path}${this.range(input)}` };
      case 'Edit':
      case 'MultiEdit':
      case 'NotebookEdit': return { kind: 'edit', text: `ändert ${file}`, detail: `${name} ${path}` };
      case 'Write': return { kind: 'edit', text: `schreibt ${file}`, detail: `Write ${path}` };
      case 'Bash':
      case 'PowerShell': {
        const command = String(input.command ?? '');
        const { kind, text } = this.shell.summarize(command, typeof input.description === 'string' ? input.description : undefined);
        return { kind, text, detail: `${name === 'Bash' ? '$' : 'PS>'} ${this.clip(command)}` };
      }
      case 'Grep': return {
        kind: 'search', text: `sucht „${this.shorten(input.pattern, 80)}“${file ? ` in ${file}` : ''}`,
        detail: `Grep ${JSON.stringify(String(input.pattern ?? ''))}${path ? ` ${path}` : ''}${typeof input.glob === 'string' ? ` --glob ${input.glob}` : ''}`,
      };
      case 'Glob': return { kind: 'search', text: `sucht Dateien ${this.shorten(input.pattern, 80)}`, detail: `Glob ${String(input.pattern ?? '')}${path ? ` ${path}` : ''}` };
      case 'TodoWrite': {
        const todos = Array.isArray(input.todos) ? (input.todos as Array<{ status?: unknown; content?: unknown }>) : [];
        const active = todos.find((t) => t?.status === 'in_progress');
        const text = `Aufgabenliste: ${todos.filter((t) => t?.status === 'completed').length}/${todos.length} erledigt` +
          (active?.content ? ` – jetzt: ${this.shorten(active.content, 120)}` : '');
        const mark = (s: unknown) => (s === 'completed' ? '[x]' : s === 'in_progress' ? '[>]' : '[ ]');
        return { kind: 'todo', text, detail: todos.map((t) => `${mark(t?.status)} ${String(t?.content ?? '')}`).join('\n') };
      }
      default: return { kind: 'tool', text: `${name}${file ? ` ${file}` : ''}`, detail: `${name} ${this.clip(JSON.stringify(input))}` };
    }
  }

  // Ergebnis eines Aufrufs: Fehler mit Meldung, Tests mit Zählung, sonst ein leeres „ok“ (färbt den Aufruf in der Oberfläche)
  private result(c: ContentBlock): RunLogLine[] {
    const ref = typeof c.tool_use_id === 'string' ? c.tool_use_id : undefined;
    const output = this.resultText(c.content);
    const tests = ref && this.tools.get(ref) === 'test' ? this.testCount(output) : '';
    const base = ref ? { ref } : {};
    if (c.is_error === true) {
      return [{ kind: 'fail', text: tests || this.shorten(output, 300), detail: this.clip(output.trim(), true), ...base }];
    }
    return [{ kind: 'ok', text: tests, ...base }];
  }

  // Zählung aus der Ausgabe von vitest/jest („Tests  3 failed | 409 passed“), über mehrere Pakete summiert; '' ohne Zählung
  private testCount(output: string): string {
    let failed = 0;
    let passed = 0;
    let found = false;
    // Farbcodes der Testausgabe – Steuerzeichen im Regex sind gewollt
    for (const m of output.replace(/\x1b\[[0-9;]*m/g, '').matchAll(/^\s*Tests:?\s+(.+)$/gm)) {
      const f = /(\d+) failed/.exec(m[1]!);
      const p = /(\d+) passed/.exec(m[1]!);
      if (!f && !p) continue;
      found = true;
      failed += Number(f?.[1] ?? 0);
      passed += Number(p?.[1] ?? 0);
    }
    if (!found) return '';
    return failed ? `${failed} fehlgeschlagen, ${passed} bestanden` : `${passed} bestanden`;
  }

  // Lesbare Zeilen als Text (Kommentare, Auswertungen): Ergebnisse ohne Text fallen weg
  private plain(e: RunLogLine): string[] {
    switch (e.kind) {
      case 'say': return [`Claude: ${e.text}`];
      case 'fail': return [`  Fehler: ${e.text}`];
      case 'ok': return e.text ? [`  ${e.text}`] : [];
      default: return [e.text];
    }
  }

  // Einträge einer Zeile: leere fallen weg, Nicht-JSON bleibt als raw
  private readable(raw: string): RunLogLine[] {
    const line = raw.replace(/\r$/, '');
    if (!line.trim()) return [];
    return this.entries(line) ?? [{ kind: 'raw', text: line }];
  }

  // „ (Zeilen a–b)“ für Read mit offset/limit
  private range(input: Input): string {
    const offset = typeof input.offset === 'number' ? input.offset : null;
    const limit = typeof input.limit === 'number' ? input.limit : null;
    if (offset === null && limit === null) return '';
    const from = offset ?? 1;
    return limit === null ? ` (ab Zeile ${from})` : ` (Zeilen ${from}–${from + limit - 1})`;
  }

  // Pfade im Arbeitsordner relativ; der Ordner selbst wird zu „.“
  private relative(text: string): string {
    if (!this.rootPattern) return text;
    return text.replace(this.rootPattern, (_m, sep: string | undefined) => (sep ? '' : '.'));
  }

  // Muster für den Ordner in allen Schreibweisen (C:\a\b, C:/a/b, /c/a/b), danach optional ein Trenner
  private pathPattern(root: string): RegExp | null {
    const norm = root.replace(/\\/g, '/').replace(/\/+$/, '');
    if (!norm) return null;
    const esc = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').replace(/\//g, '[\\\\/]');
    const drive = /^([A-Za-z]):\/(.*)$/.exec(norm);
    const head = drive ? `(?:${drive[1]}:[\\\\/]|/${drive[1]}/)${esc(drive[2]!)}` : esc(norm);
    return new RegExp(`${head}([\\\\/])?`, 'gi');
  }

  // JSON-Objekt der Zeile oder null
  private parse(line: string): StreamEvent | null {
    const text = line.trim();
    if (!text.startsWith('{')) return null;
    try {
      const parsed = JSON.parse(text) as unknown;
      return parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? (parsed as StreamEvent) : null;
    } catch {
      return null;
    }
  }

  // Text eines Werkzeugergebnisses (Text oder Liste von Textblöcken)
  private resultText(content: unknown): string {
    if (Array.isArray(content)) return content.map((x: { text?: unknown }) => (typeof x?.text === 'string' ? x.text : '')).join('\n');
    return String(content ?? '');
  }

  // Eine Zeile, Leerraum zusammengezogen, höchstens `max` Zeichen
  private shorten(text: unknown, max: number): string {
    const one = String(text ?? '').replace(/\s+/g, ' ').trim();
    return one.length > max ? `${one.slice(0, max - 1)}…` : one;
  }

  // Mehrzeilig, höchstens DETAIL_MAX Zeichen; `tail` behält das Ende (Testausgaben fassen am Schluss zusammen)
  private clip(text: string, tail = false): string {
    if (text.length <= DETAIL_MAX) return text;
    return tail ? `…${text.slice(-(DETAIL_MAX - 1))}` : `${text.slice(0, DETAIL_MAX - 1)}…`;
  }
}
