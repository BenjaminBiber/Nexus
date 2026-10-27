import { appendFile, mkdir } from 'node:fs/promises';
import { dirname } from 'node:path';

// Schreibt Umgebungsvariablen in die Datei, die Claude Code (CLAUDE_ENV_FILE) für die Session einliest.
export class EnvFileWriter {
  // Hängt `export NAME='wert'` an; bereits gesetzte Variablen bleiben unberührt. Liefert die Zahl der Zeilen.
  async append(file: string | undefined, vars: Record<string, string>, existing: Record<string, string | undefined>): Promise<number> {
    if (!file) return 0;
    const lines = Object.entries(vars)
      .filter(([name]) => !existing[name])
      .map(([name, value]) => `export ${name}=${this.quote(value)}\n`);
    if (lines.length === 0) return 0;
    await mkdir(dirname(file), { recursive: true });
    await appendFile(file, lines.join(''), 'utf8');
    return lines.length;
  }

  // Einfache Anführungszeichen, eingebettete werden maskiert
  private quote(value: string): string {
    return `'${value.replace(/'/g, "'\\''")}'`;
  }
}
