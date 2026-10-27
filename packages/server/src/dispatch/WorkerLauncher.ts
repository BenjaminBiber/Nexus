import { spawn } from 'node:child_process';
import { isSea } from 'node:sea';

// Startet den Worker eines Laufs; Tests ersetzen das durch eine Attrappe.
export interface WorkerLauncher {
  // Startet losgelöst und liefert die Prozess-ID (null, wenn unbekannt)
  launch(runId: string, env: NodeJS.ProcessEnv): number | null;
}

// Startet dieselbe Programmdatei wie der Server (gebündelt oder per tsx) mit --dispatch-worker; überlebt einen Neustart des Servers.
export class ProcessWorkerLauncher implements WorkerLauncher {
  // sea: läuft der Server als gebündeltes Sidecar? Dann ist process.execPath das Programm selbst
  // und es gibt keine Programmdatei als Argument. Tests setzen den Wert selbst.
  constructor(
    private readonly entry: string = process.argv[1],
    private readonly execArgv: string[] = process.execArgv,
    private readonly sea: boolean = isSea(),
  ) {}

  // Losgelöster Prozess ohne Fenster und ohne Ausgabe; Debug-Schalter des Servers gehen nicht mit
  // (sonst bliebe der Worker am belegten Debug-Port hängen). Startfehler zeigen sich als fehlende PID
  launch(runId: string, env: NodeJS.ProcessEnv): number | null {
    const child = spawn(process.execPath, [...this.prefix(), '--dispatch-worker', runId], {
      detached: true, stdio: 'ignore', windowsHide: true, env,
    });
    child.on('error', () => undefined);
    child.unref();
    return child.pid ?? null;
  }

  // Was vor --dispatch-worker steht: im Sidecar nichts, sonst die Schalter von Node und die Programmdatei
  private prefix(): string[] {
    if (this.sea) return [];
    return [...this.execArgv.filter((arg) => !/^--(inspect|debug)/.test(arg)), this.entry];
  }
}
