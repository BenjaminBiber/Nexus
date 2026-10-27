/** Ein Testlauf des Agenten: Befehl, Ergebnis (null = ohne Ergebnis) und Ende der Ausgabe */
export interface TestRun {
  command: string;
  ok: boolean | null;
  output: string;
}

interface ContentBlock {
  type?: unknown;
  id?: unknown;
  name?: unknown;
  input?: unknown;
  tool_use_id?: unknown;
  is_error?: unknown;
  content?: unknown;
  text?: unknown;
}

/** Höchstens so viele Zeichen der Ausgabe je Testlauf (das Ende bleibt) */
const MAX_OUTPUT = 4000;

/** Werkzeuge, deren Aufrufe Befehle ausführen */
const SHELL_TOOLS = new Set(['Bash', 'PowerShell']);

/** Anfang eines Befehls: Zeilenanfang oder nach ; & | (, optional npx, python -m oder ./ davor */
const START = String.raw`(?:^|[;&|(]\s*)(?:npx\s+|python3?\s+-m\s+|\.\/)?`;

/** Befehle, die als Testlauf gelten (Tests, Build, Lint, Typprüfung) */
const TEST_PATTERNS = [
  String.raw`(vitest|jest|pytest)\b`,
  String.raw`npm\s+(test|t)\b`,
  String.raw`npm\s+run\s+(test|build|lint|typecheck)\b`,
  String.raw`tsc\b`,
  String.raw`dotnet\s+test\b`,
  String.raw`go\s+test\b`,
  String.raw`cargo\s+test\b`,
  String.raw`(mvn|mvnw|gradle|gradlew)\b.*\btest\b`,
].map((p) => new RegExp(START + p));

// Liest die Testläufe des Agenten aus der Ausgabe von `claude -p --output-format stream-json`.
export class TestRunExtractor {
  // Testläufe in der Reihenfolge der Aufrufe; Zeilen ohne JSON fallen weg
  extract(output: string): TestRun[] {
    const runs = new Map<string, TestRun>();
    for (const line of output.split('\n')) {
      for (const block of this.blocks(line)) {
        if (block.type === 'tool_use' && SHELL_TOOLS.has(String(block.name)) && typeof block.id === 'string') {
          const command = this.command(block.input);
          if (command && this.isTest(command)) runs.set(block.id, { command, ok: null, output: '' });
        } else if (block.type === 'tool_result' && typeof block.tool_use_id === 'string') {
          const run = runs.get(block.tool_use_id);
          if (!run) continue;
          run.ok = block.is_error !== true;
          run.output = this.text(block.content).slice(-MAX_OUTPUT);
        }
      }
    }
    return [...runs.values()];
  }

  // Ist der Befehl ein Testlauf?
  isTest(command: string): boolean {
    return TEST_PATTERNS.some((p) => p.test(command));
  }

  // Inhaltsblöcke einer Zeile (assistant/user); leer, wenn die Zeile kein solches Ereignis ist
  private blocks(line: string): ContentBlock[] {
    const text = line.trim();
    if (!text.startsWith('{')) return [];
    try {
      const ev = JSON.parse(text) as { message?: { content?: unknown } };
      const content = ev?.message?.content;
      return Array.isArray(content) ? (content.filter((c) => c && typeof c === 'object') as ContentBlock[]) : [];
    } catch {
      return [];
    }
  }

  // Befehl eines Werkzeugaufrufs
  private command(input: unknown): string {
    const cmd = input && typeof input === 'object' ? (input as { command?: unknown }).command : undefined;
    return typeof cmd === 'string' ? cmd.trim() : '';
  }

  // Text eines Ergebnisses: String oder Liste von Textblöcken
  private text(content: unknown): string {
    if (typeof content === 'string') return content;
    if (!Array.isArray(content)) return '';
    return content
      .map((c) => (c && typeof c === 'object' && typeof (c as ContentBlock).text === 'string' ? ((c as ContentBlock).text as string) : ''))
      .filter(Boolean)
      .join('\n');
  }
}
