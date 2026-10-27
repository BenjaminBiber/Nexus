import { execFile, execFileSync } from 'node:child_process';
import { HarnessError } from '../errors/HarnessError.js';

// Größte Ausgabe eines Git-Aufrufs (große Status- oder Diff-Listen)
const MAX_BUFFER = 64 * 1024 * 1024;

// Ergebnis eines Git-Aufrufs, der scheitern darf
export interface GitAttempt {
  ok: boolean;
  out: string;
  err: string;
}

// Führt Git-Befehle mit einer Argumentliste aus (keine Shell, daher sicher bei Leerzeichen).
export class GitRunner {
  // Führt `git <args>` im Ordner `cwd` aus und liefert die getrimmte Ausgabe
  run(cwd: string, args: string[]): string {
    return this.raw(cwd, args).trim();
  }

  // Wie run, aber ohne den Prozess zu blockieren (für den Server: ein Git-Start dauert unter Windows einige 100 ms)
  runAsync(cwd: string, args: string[]): Promise<string> {
    return new Promise((resolve, reject) => {
      execFile('git', args, { cwd, encoding: 'utf8', windowsHide: true, maxBuffer: MAX_BUFFER }, (error, stdout, stderr) => {
        if (!error) return resolve(stdout.trim());
        const detail = stderr.trim() || String(error);
        reject(new HarnessError('git-failed', `Git-Befehl fehlgeschlagen (git ${args.join(' ')}): ${detail}`));
      });
    });
  }

  // Wie run, aber ungetrimmt (für maschinenlesbare Ausgaben, deren erste Spalte ein Leerzeichen sein kann)
  raw(cwd: string, args: string[]): string {
    try {
      return execFileSync('git', args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true, maxBuffer: MAX_BUFFER });
    } catch (error) {
      const detail = (error as { stderr?: string }).stderr?.toString().trim() || String(error);
      throw new HarnessError('git-failed', `Git-Befehl fehlgeschlagen (git ${args.join(' ')}): ${detail}`);
    }
  }

  // Führt aus, ohne zu werfen; liefert Erfolg, Ausgabe und Fehlerausgabe (getrimmt)
  tryRun(cwd: string, args: string[]): GitAttempt {
    try {
      const out = execFileSync('git', args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true, maxBuffer: MAX_BUFFER });
      return { ok: true, out: out.trim(), err: '' };
    } catch (error) {
      const e = error as { stdout?: string | Buffer; stderr?: string | Buffer };
      return { ok: false, out: e.stdout?.toString().trim() ?? '', err: e.stderr?.toString().trim() || String(error) };
    }
  }
}
