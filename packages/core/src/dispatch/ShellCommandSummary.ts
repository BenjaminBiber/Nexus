/** Art eines Shell-Befehls im Lauf-Protokoll */
export type ShellKind = 'read' | 'edit' | 'search' | 'test' | 'run';

/** Kurzfassung eines Shell-Befehls */
export interface ShellSummary {
  kind: ShellKind;
  text: string;
}

const READERS = new Set(['cat', 'head', 'tail', 'less', 'nl', 'wc', 'type']);
const TEST = /\b(vitest|jest|playwright test|npm (?:run )?test|npm t\b|dotnet test|pytest)\b/;
const INSTALL = /^(?:npm (?:ci|install|i)|pnpm install|yarn(?: install)?)(?:\s|$)/;
const SCRIPT = /^(?:node|python3?|py|tsx|npx tsx|deno run)\b/;
const WRITES = /writeFileSync|writeFile\(|appendFileSync|open\([^)]*['"][wa]['"]|\.write_text\(|\.write\(/;
const FILE_LITERAL = /['"`]([^'"`\s]+\.(?:tsx?|jsx?|mjs|cjs|css|scss|json|md|html|py|cs|ya?ml|sql|txt))['"`]/g;

// Macht aus einem Shell-Befehl des Agenten eine lesbare Zeile: was er tut (lesen, ändern, suchen, testen, ausführen) und woran.
// Bewusst grob – im Zweifel bleibt der gekürzte Befehl. `strip` kürzt Pfade (z. B. auf den Worktree).
export class ShellCommandSummary {
  constructor(private readonly strip: (text: string) => string = (t) => t) {}

  // Kurzfassung; eine Beschreibung des Agenten (Feld `description` des Bash-Tools) ersetzt den Text, nicht die Art
  summarize(command: string, description?: string): ShellSummary {
    const own = this.guess(command);
    const text = description?.trim() ? this.one(description, 160) : own.text;
    return { kind: own.kind, text };
  }

  private guess(command: string): ShellSummary {
    const full = this.strip(command);
    const first = this.withoutCd(full.split('\n')[0] ?? '');
    if (TEST.test(first)) return { kind: 'test', text: this.testText(first) };
    if (INSTALL.test(first)) return { kind: 'run', text: 'installiert Abhängigkeiten' };
    if (SCRIPT.test(first) && (/<</.test(first) || /\s-e\s/.test(first) || /\s-c\s/.test(first))) {
      const files = [...new Set([...full.matchAll(FILE_LITERAL)].map((m) => m[1]!))];
      const list = files.length ? `: ${this.list(files)}` : '';
      return WRITES.test(full) ? { kind: 'edit', text: `ändert per Skript${list}` } : { kind: 'run', text: `führt Skript aus${list}` };
    }
    const write = /^cat\s+(?:<<-?\s*['"]?\w+['"]?\s+)?>\s*(\S+)/.exec(first) ?? /^cat\s+>\s*(\S+)/.exec(first) ?? /\btee\s+(?:-a\s+)?(\S+)/.exec(first);
    if (write) return { kind: 'edit', text: `schreibt ${this.list([write[1]!])}` };
    const segments = first.split(/\s*(?:&&|\|\||;)\s*/).map((s) => this.withoutCd(s)).filter((s) => s && !/^echo\b|^printf\b/.test(s));
    if (segments.length && segments.every((s) => /^sed\s+-i/.test(s))) {
      return { kind: 'edit', text: `ändert ${this.list(this.files(segments, (t) => t.slice(-1)))}` };
    }
    const piped = segments.map((s) => s.split(/\s*\|\s*/)[0]!);
    if (piped.length && piped.every((s) => this.isRead(s))) {
      // Bei sed ist das erste Wort ohne Bindestrich das Skript, danach kommen die Dateien
      const files = this.files(piped, (t) => t.slice(1).filter((x) => !x.startsWith('-')).slice(t[0] === 'sed' ? 1 : 0));
      if (files.length) return { kind: 'read', text: `liest ${this.list(files)}` };
    }
    const lead = piped[0] ?? first;
    const grep = /^(?:grep|rg|findstr)\b(.*)$/.exec(lead);
    if (grep) {
      const args = this.tokens(grep[1]!).filter((t) => !t.startsWith('-'));
      const where = args.length > 1 ? ` in ${this.list(args.slice(1))}` : '';
      return { kind: 'search', text: `sucht „${this.one(args[0] ?? '', 60)}“${where}` };
    }
    if (/^(?:ls|dir|file|stat|test|find)\b/.test(lead)) return { kind: 'run', text: 'prüft Dateien' };
    const git = /^git\s+(\S+)(.*)$/.exec(lead);
    if (git) {
      const message = git[1] === 'commit' ? /-m\s+(["'])(.*?)\1/.exec(git[2]!)?.[2] : undefined;
      return { kind: 'run', text: message ? `git commit „${this.one(message, 80)}“` : `git ${git[1]}` };
    }
    const harness = /^harness\s+board\s+(\S+)\s*(\S+)?(.*)$/.exec(lead);
    if (harness) return { kind: 'run', text: this.harnessText(harness[1]!, harness[2], harness[3]!) };
    return { kind: 'run', text: this.one(first, 120) };
  }

  // „Tests“ mit den genannten Testdateien, sonst „(alle)“
  private testText(command: string): string {
    const files = this.tokens(command).filter((t) => /\.(?:test|spec)\.[cm]?[tj]sx?$/.test(t) || /(?:^|\/)tests?\//.test(t));
    return files.length ? `Tests: ${this.list(files)}` : 'Tests (alle)';
  }

  private harnessText(verb: string, id: string | undefined, rest: string): string {
    const step = /^\s*(\d+)/.exec(rest)?.[1];
    switch (verb) {
      case 'show': return `liest Ticket ${id ?? ''}`.trim();
      case 'comment': return `kommentiert ${id ?? ''}`.trim();
      case 'step': return `hakt Schritt ${step ?? '?'} ab`;
      case 'done': return id ? `meldet ${id} als erledigt` : 'meldet erledigt';
      default: return this.one(`harness board ${verb} ${id ?? ''}${rest}`, 120);
    }
  }

  // Liest das Segment nur (cat, head, sed -n …, ohne Umleitung in eine Datei)?
  private isRead(segment: string): boolean {
    if (/>/.test(segment)) return false;
    const cmd = segment.split(/\s+/)[0] ?? '';
    return READERS.has(cmd) || (cmd === 'sed' && /\s-n\s/.test(` ${segment} `) && !/\s-i/.test(segment));
  }

  // Dateien aus den Segmenten (je Segment `pick` auf die Wörter), ohne Doppelte
  private files(segments: string[], pick: (tokens: string[]) => string[]): string[] {
    const out: string[] = [];
    for (const s of segments) for (const a of pick(this.tokens(s))) if (!out.includes(a)) out.push(a);
    return out;
  }

  // Höchstens drei Namen (Dateinamen ohne Ordner), Rest als „+N“
  private list(items: string[]): string {
    const names = items.map((i) => this.unquote(i).replace(/\*$/, '').split('/').pop() || i);
    return names.length > 3 ? `${names.slice(0, 3).join(', ')} +${names.length - 3}` : names.join(', ');
  }

  // Führende „cd … &&“ weg
  private withoutCd(segment: string): string {
    let s = segment.trim();
    for (let m = /^cd\s+\S+\s*(?:&&|;)\s*/.exec(s); m; m = /^cd\s+\S+\s*(?:&&|;)\s*/.exec(s)) s = s.slice(m[0].length);
    return s;
  }

  // Wörter mit einfachen und doppelten Anführungszeichen
  private tokens(text: string): string[] {
    return [...text.matchAll(/'[^']*'|"[^"]*"|\S+/g)].map((m) => this.unquote(m[0]));
  }

  private unquote(text: string): string {
    return /^(['"]).*\1$/.test(text) ? text.slice(1, -1) : text;
  }

  private one(text: string, max: number): string {
    const one = text.replace(/\s+/g, ' ').trim();
    return one.length > max ? `${one.slice(0, max - 1)}…` : one;
  }
}
