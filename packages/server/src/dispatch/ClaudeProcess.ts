import { spawn, spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { delimiter, extname, join, sep } from 'node:path';
import { HarnessError, type RunResult } from '@harness/core';

const MAX_OUTPUT_CHARS = 200_000;

/** Herkunft einer Ausgabe: Claude (stdout, stderr) oder eine Meldung des Dispatchers selbst */
export type OutputSource = 'stdout' | 'stderr' | 'dispatch';

export interface ClaudeRunInput {
  command: string;
  args: string[];
  /** Arbeitsanweisung; geht über stdin an Claude */
  prompt: string;
  cwd: string;
  env: NodeJS.ProcessEnv;
  timeoutMs: number;
  /** Jede Ausgabe fürs Protokoll mit ihrer Quelle (stdout, stderr oder Meldung des Dispatchers) */
  onOutput: (chunk: string, source: OutputSource) => void;
  /** Bricht den Lauf ab: Claude wird samt Kindprozessen beendet, das Ergebnis kommt wie sonst */
  signal?: AbortSignal;
}

// Startet Claude Code im Headless-Modus, beendet es nach dem Zeitlimit samt Kindprozessen und sammelt die Ausgabe.
export class ClaudeProcess {
  // hardKillMs: Wartezeit nach dem Beenden im Zeitlimit, bis hart beendet und das Ergebnis geliefert wird
  constructor(private readonly hardKillMs: number = 15_000) {}

  // Führt den Lauf aus; wirft nie, Fehler landen im Ergebnis (Befehl nicht gefunden: Exit-Code 127)
  run(input: ClaudeRunInput): Promise<RunResult> {
    let child: ReturnType<typeof spawn>;
    try {
      const { file, shell } = this.resolveCommand(input.command, input.env);
      const options = { cwd: input.cwd, env: input.env, windowsHide: true, detached: process.platform !== 'win32' };
      child = shell
        ? spawn([this.quote(file), ...input.args.map((a) => this.quote(a))].join(' '), { ...options, shell: true })
        : spawn(file, input.args, options);
    } catch (error) {
      return Promise.resolve({ code: 127, timedOut: false, output: (error as Error).message });
    }
    return new Promise((resolve) => {
      let stdout = '';
      let stderr = '';
      let timedOut = false;
      let settled = false;
      // Nur das Ende behalten; die angeschnittene erste Zeile fällt weg, damit kein JSON-Bruchstück als Fehlertext zählt
      const keep = (text: string) => {
        if (text.length <= MAX_OUTPUT_CHARS) return text;
        const tail = text.slice(-MAX_OUTPUT_CHARS);
        const lineStart = tail.indexOf('\n');
        return lineStart >= 0 ? tail.slice(lineStart + 1) : tail;
      };
      let hardTimer: NodeJS.Timeout | undefined;
      const timer = setTimeout(() => {
        timedOut = true;
        input.onOutput('\n[dispatch] Zeitlimit erreicht – Claude wird beendet\n', 'dispatch');
        this.killTree(child.pid);
        // Endet der Prozess trotzdem nicht, hart beenden und nicht länger warten
        hardTimer = setTimeout(() => {
          try {
            child.kill('SIGKILL');
          } catch {
            /* schon beendet */
          }
          finish(null);
        }, this.hardKillMs);
        hardTimer.unref();
      }, input.timeoutMs);
      const finish = (code: number | null) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        if (hardTimer) clearTimeout(hardTimer);
        resolve({ code, timedOut, output: [stdout, stderr].filter((s) => s.trim()).join('\n') });
      };
      // Als Text dekodieren, damit über Blockgrenzen geteilte UTF-8-Zeichen heil bleiben
      child.stdout?.setEncoding('utf8');
      child.stderr?.setEncoding('utf8');
      child.stdout?.on('data', (text: string) => {
        stdout = keep(stdout + text);
        input.onOutput(text, 'stdout');
      });
      child.stderr?.on('data', (text: string) => {
        stderr = keep(stderr + text);
        input.onOutput(text, 'stderr');
      });
      child.on('error', (error) => {
        stderr += `\n${error.message}`;
        finish(127);
      });
      child.on('close', (code) => finish(code));
      // Hält ein übrig gebliebener Kindprozess die Ausgabe offen, nicht ewig auf „close“ warten
      child.on('exit', (code) => setTimeout(() => finish(code), 10_000).unref());
      // Abbruch von außen (z. B. „Abbrechen“ in der Oberfläche) beendet Claude wie das Zeitlimit
      const abort = () => {
        input.onOutput('\n[dispatch] Abgebrochen – Claude wird beendet\n', 'dispatch');
        this.killTree(child.pid);
      };
      if (input.signal?.aborted) abort();
      else input.signal?.addEventListener('abort', abort, { once: true });
      child.on('close', () => input.signal?.removeEventListener('abort', abort));
      child.stdin?.on('error', () => undefined);
      child.stdin?.end(input.prompt);
    });
  }

  // Unter Windows startet Node eine .cmd nur über die Shell, die .exe direkt; ein Pfad wird genommen, wie er ist
  private resolveCommand(command: string, env: NodeJS.ProcessEnv): { file: string; shell: boolean } {
    if (process.platform !== 'win32' || extname(command) || command.includes(sep) || command.includes('/')) {
      if ((command.includes(sep) || command.includes('/')) && !existsSync(command)) {
        throw new HarnessError('claude-not-found', `Claude Code nicht gefunden: ${command}`);
      }
      return { file: command, shell: /\.(cmd|bat)$/i.test(command) };
    }
    const dirs = (env.PATH ?? env.Path ?? '').split(delimiter).filter(Boolean);
    // Wie Windows selbst: zuerst die Reihenfolge der PATH-Ordner, je Ordner .exe vor .cmd vor .bat
    for (const dir of dirs) {
      for (const ext of ['.exe', '.cmd', '.bat']) {
        const candidate = join(dir, command + ext);
        if (existsSync(candidate)) return { file: candidate, shell: ext !== '.exe' };
      }
    }
    return { file: command, shell: false };
  }

  // Für cmd.exe quoten; die Argumente stammen aus der Einstellung, nicht aus Tickets
  private quote(arg: string): string {
    if (/["%!^]/.test(arg)) throw new HarnessError('claude-arg-unsafe', `Argument für claude enthält ein Zeichen, das cmd.exe nicht sicher weitergibt: ${arg}`);
    return /^[\w\-.:/\\=]+$/.test(arg) ? arg : `"${arg}"`;
  }

  // Beendet Claude samt allem, was es gestartet hat (Tests, Builds)
  private killTree(pid: number | undefined): void {
    if (!pid) return;
    if (process.platform === 'win32') {
      spawnSync('taskkill', ['/pid', String(pid), '/T', '/F'], { stdio: 'ignore', windowsHide: true });
    } else {
      try {
        process.kill(-pid, 'SIGKILL');
      } catch {
        /* schon beendet */
      }
    }
  }
}
