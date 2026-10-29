import { execFile, type ExecFileOptions } from 'node:child_process';
import { readlink, realpath, stat } from 'node:fs/promises';
import { join, sep } from 'node:path';
import { promisify } from 'node:util';

// Erst beim Aufruf gebunden: Tests, die `node:child_process` nachbilden, laden den Kern so ohne `execFile`
const run = (file: string, args: string[], options: ExecFileOptions): Promise<{ stdout: string }> =>
  promisify(execFile)(file, args, { ...options, encoding: 'utf8' }) as Promise<{ stdout: string }>;

/** Ein laufender Prozess; `cwd` nur, wo das System ihn preisgibt (nicht unter Windows) */
export interface ProcessInfo {
  pid: number;
  ppid: number;
  command: string;
  cwd?: string | null;
}

/** Zugriff auf die Prozesse des Rechners; Tests setzen eigene Funktionen ein */
export interface ProcessSystem {
  list(): Promise<ProcessInfo[]>;
  cwds(pids: number[]): Promise<Map<number, string>>;
  signal(pid: number, signal: 'SIGTERM' | 'SIGKILL'): void;
  alive(pid: number): boolean;
  sleep(ms: number): Promise<void>;
}

const TERM_WAIT_MS = 3000;
const POLL_MS = 100;

// Räumt nach einem Dispatch-Lauf Prozesse ab, die aus dessen Worktree übrig geblieben sind – etwa ein
// `nexus serve … &`, das der Agent für eine Prüfung gestartet und nie beendet hat. Beendet werden nur
// verwaiste Prozesse (Elternprozess beendet) samt ihren Kindern; was noch an einer Shell oder IDE hängt, bleibt.
export class OrphanSweeper {
  constructor(private readonly system: ProcessSystem = OrphanSweeper.local()) {}

  // Beendet die übrig gebliebenen Prozesse aus `root` (erst SIGTERM, nach einer Frist SIGKILL); liefert ihre PIDs
  async sweep(root: string, self: number = process.pid): Promise<number[]> {
    const all = await this.system.list();
    const orphans = all.filter((p) => OrphanSweeper.orphaned(p, all) && p.pid !== self);
    const cwds = orphans.length && process.platform !== 'win32' ? await this.system.cwds(orphans.map((p) => p.pid)) : new Map<number, string>();
    const withCwd = all.map((p) => (cwds.has(p.pid) ? { ...p, cwd: cwds.get(p.pid) } : p));
    // lsof und /proc melden echte Pfade (unter macOS etwa /private/var statt /var)
    const real = await realpath(root).catch(() => root);
    const doomed = [...new Set([root, real].flatMap((r) => OrphanSweeper.select(withCwd, r, self)))];
    if (!doomed.length) return [];
    for (const pid of doomed) this.system.signal(pid, 'SIGTERM');
    for (let waited = 0; waited < TERM_WAIT_MS && doomed.some((pid) => this.system.alive(pid)); waited += POLL_MS) {
      await this.system.sleep(POLL_MS);
    }
    for (const pid of doomed) if (this.system.alive(pid)) this.system.signal(pid, 'SIGKILL');
    return doomed;
  }

  // Ist `dir` ein verknüpfter Worktree (`.git` ist eine Datei)? Im Hauptordner wird nie aufgeräumt
  static async linkedWorktree(dir: string): Promise<boolean> {
    try {
      return (await stat(join(dir, '.git'))).isFile();
    } catch {
      return false;
    }
  }

  // Die zu beendenden Prozesse: verwaiste Prozesse aus `root` und alle ihre Nachfahren, nie `self` oder seine Vorfahren
  static select(processes: ProcessInfo[], root: string, self: number): number[] {
    const byPid = new Map(processes.map((p) => [p.pid, p]));
    const protectedPids = new Set<number>();
    for (let at = byPid.get(self); at && !protectedPids.has(at.pid); at = byPid.get(at.ppid)) protectedPids.add(at.pid);
    const roots = processes.filter((p) => OrphanSweeper.orphaned(p, processes) && OrphanSweeper.fromRoot(p, root));
    const chosen = new Set<number>();
    const queue = roots.map((p) => p.pid);
    while (queue.length) {
      const pid = queue.shift()!;
      if (chosen.has(pid) || protectedPids.has(pid)) continue;
      chosen.add(pid);
      for (const p of processes) if (p.ppid === pid) queue.push(p.pid);
    }
    return [...chosen];
  }

  // Verwaist: der Elternprozess ist init/launchd (PID 1) oder läuft nicht mehr (Windows hängt Waisen nicht um)
  static orphaned(p: ProcessInfo, processes: ProcessInfo[]): boolean {
    if (p.pid <= 1) return false;
    return p.ppid === 1 || !processes.some((q) => q.pid === p.ppid);
  }

  // Stammt der Prozess aus `root`: Pfad in der Befehlszeile oder Arbeitsordner darin
  static fromRoot(p: ProcessInfo, root: string): boolean {
    const base = root.replace(/[\\/]+$/, '');
    const inside = (path: string) => path === base || path.startsWith(base + sep) || path.startsWith(base + '/');
    if (p.cwd && inside(p.cwd)) return true;
    const at = p.command.indexOf(base);
    if (at < 0) return false;
    const next = p.command.charAt(at + base.length);
    return next === '' || next === '/' || next === '\\' || next === ' ' || next === '"' || next === "'";
  }

  // Prozesse dieses Rechners: `ps` unter Unix, CIM unter Windows; Arbeitsordner über /proc bzw. lsof
  static local(): ProcessSystem {
    return {
      async list() {
        if (process.platform === 'win32') {
          const script = 'Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,CommandLine | ConvertTo-Json -Compress';
          const { stdout } = await run('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', script], { windowsHide: true, maxBuffer: 64 * 1024 * 1024 });
          const rows = JSON.parse(stdout || '[]') as Array<{ ProcessId: number; ParentProcessId: number; CommandLine: string | null }>;
          return (Array.isArray(rows) ? rows : [rows]).map((r) => ({ pid: r.ProcessId, ppid: r.ParentProcessId, command: r.CommandLine ?? '' }));
        }
        const { stdout } = await run('ps', ['-A', '-o', 'pid=,ppid=,command='], { maxBuffer: 64 * 1024 * 1024 });
        return stdout.split('\n').flatMap((line) => {
          const m = /^\s*(\d+)\s+(\d+)\s(.*)$/.exec(line);
          return m ? [{ pid: Number(m[1]), ppid: Number(m[2]), command: m[3].trim() }] : [];
        });
      },
      async cwds(pids) {
        const found = new Map<number, string>();
        if (!pids.length) return found;
        if (process.platform === 'linux') {
          for (const pid of pids) {
            try {
              found.set(pid, await readlink(`/proc/${pid}/cwd`));
            } catch {
              // Prozess weg oder kein Zugriff
            }
          }
          return found;
        }
        let stdout: string;
        try {
          ({ stdout } = await run('lsof', ['-a', '-d', 'cwd', '-Fn', '-p', pids.join(',')], { maxBuffer: 16 * 1024 * 1024 }));
        } catch (error) {
          // lsof endet mit Code 1, sobald ein Prozess fehlt; die Ausgabe gilt trotzdem
          stdout = (error as { stdout?: string }).stdout ?? '';
        }
        let pid = 0;
        for (const line of stdout.split('\n')) {
          if (line.startsWith('p')) pid = Number(line.slice(1));
          else if (line.startsWith('n') && pid) found.set(pid, line.slice(1));
        }
        return found;
      },
      signal(pid, signal) {
        try {
          if (process.platform === 'win32') {
            void run('taskkill', ['/pid', String(pid), '/T', '/F'], { windowsHide: true }).catch(() => undefined);
          } else {
            process.kill(pid, signal);
          }
        } catch {
          // schon beendet
        }
      },
      alive(pid) {
        try {
          process.kill(pid, 0);
          return true;
        } catch {
          return false;
        }
      },
      sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
    };
  }
}
