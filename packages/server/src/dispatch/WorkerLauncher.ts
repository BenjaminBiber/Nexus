import { spawn } from 'node:child_process';

// Startet den Worker eines Laufs; Tests ersetzen das durch eine Attrappe.
export interface WorkerLauncher {
  // Startet losgelöst und liefert die Prozess-ID (null, wenn unbekannt)
  launch(runId: string, env: NodeJS.ProcessEnv): number | null;
}

// Startet dieselbe Programmdatei wie der Server (gebündelt oder per tsx) mit --dispatch-worker; überlebt einen Neustart des Servers.
export class ProcessWorkerLauncher implements WorkerLauncher {
  constructor(
    private readonly entry: string = process.argv[1],
    private readonly execArgv: string[] = process.execArgv,
  ) {}

  // Losgelöster Prozess ohne Fenster und ohne Ausgabe; Debug-Schalter des Servers gehen nicht mit
  // (sonst bliebe der Worker am belegten Debug-Port hängen). Startfehler zeigen sich als fehlende PID
  launch(runId: string, env: NodeJS.ProcessEnv): number | null {
    const execArgv = this.execArgv.filter((arg) => !/^--(inspect|debug)/.test(arg));
    const child = spawn(process.execPath, [...execArgv, this.entry, '--dispatch-worker', runId], {
      detached: true, stdio: 'ignore', windowsHide: true, env,
    });
    child.on('error', () => undefined);
    child.unref();
    return child.pid ?? null;
  }
}
