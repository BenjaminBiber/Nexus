import { spawn, spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { relative, resolve, isAbsolute } from 'node:path';
import type { RunResult, WorktreeSetupStep } from '@harness/core';

const MAX_OUTPUT_CHARS = 20_000;

export interface SetupInput {
  steps: WorktreeSetupStep[];
  /** Worktree des Laufs; cwd der Schritte ist relativ dazu */
  root: string;
  env: NodeJS.ProcessEnv;
  timeoutMs: number;
  /** Ausgabe fürs Lauf-Protokoll (Kopfzeilen und Ausgabe der Befehle) */
  onOutput: (text: string) => void;
}

// Führt im Worker die Einrichtungsbefehle eines neu angelegten Worktrees nacheinander aus (vor Claude). Shell wie für Claude:
// unter Windows die Git Bash aus CLAUDE_CODE_GIT_BASH_PATH, sonst cmd; sonst /bin/sh. Beim ersten Fehlschlag (Exit ≠ 0,
// Zeitlimit) ist Schluss; das Ergebnis nennt den Befehl.
export class WorktreeSetupRunner {
  // Liefert setupOk oder setupFailed für das Ergebnis des Laufs; wirft nie
  async run(input: SetupInput): Promise<Pick<RunResult, 'setupOk' | 'setupFailed'>> {
    for (const step of input.steps) {
      input.onOutput(`[dispatch] Einrichtung: ${step.cwd}$ ${step.run}\n`);
      const cwd = resolve(input.root, step.cwd);
      const rel = relative(resolve(input.root), cwd);
      if (rel.startsWith('..') || isAbsolute(rel) || !existsSync(cwd)) {
        const output = `Ordner ${step.cwd} gibt es im Worktree nicht.`;
        input.onOutput(`[dispatch] ${output}\n`);
        return { setupFailed: { ...step, code: null, timedOut: false, output } };
      }
      const ended = await this.exec(step.run, cwd, input);
      input.onOutput(`[dispatch] Einrichtung beendet: Exit-Code ${ended.code}${ended.timedOut ? ', Zeitlimit' : ''}\n`);
      if (ended.code !== 0 || ended.timedOut) return { setupFailed: { ...step, ...ended } };
    }
    return { setupOk: true };
  }

  // Ein Befehl in der Shell; sammelt das Ende der Ausgabe
  private exec(command: string, cwd: string, input: SetupInput): Promise<{ code: number | null; timedOut: boolean; output: string }> {
    return new Promise((done) => {
      let output = '';
      let timedOut = false;
      let settled = false;
      const bash = process.platform === 'win32' ? input.env.CLAUDE_CODE_GIT_BASH_PATH : '/bin/sh';
      const options = { cwd, env: input.env, windowsHide: true, detached: process.platform !== 'win32' };
      let child: ReturnType<typeof spawn>;
      try {
        child = bash && existsSync(bash) ? spawn(bash, ['-c', command], options) : spawn(command, { ...options, shell: true });
      } catch (error) {
        done({ code: 127, timedOut: false, output: (error as Error).message });
        return;
      }
      const finish = (code: number | null) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        done({ code, timedOut, output });
      };
      const add = (text: string) => {
        output = (output + text).slice(-MAX_OUTPUT_CHARS);
        input.onOutput(text);
      };
      const timer = setTimeout(() => {
        timedOut = true;
        input.onOutput('\n[dispatch] Zeitlimit der Einrichtung erreicht – Befehl wird beendet\n');
        this.killTree(child.pid);
        setTimeout(() => finish(null), 5_000).unref();
      }, input.timeoutMs);
      child.stdout?.setEncoding('utf8');
      child.stderr?.setEncoding('utf8');
      child.stdout?.on('data', add);
      child.stderr?.on('data', add);
      child.on('error', (error) => {
        add(`\n${error.message}\n`);
        finish(127);
      });
      child.on('close', (code) => finish(code));
      child.stdin?.end();
    });
  }

  // Beendet den Befehl samt Kindprozessen
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
