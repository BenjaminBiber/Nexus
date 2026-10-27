import { ServerInfoFile } from '../registry/ServerInfoFile.js';
import type { HarnessHome } from '../registry/HarnessHome.js';

export interface StopDeps {
  /** Antwortet unter diesem Port ein Harness-Server? */
  isOurs(port: number): Promise<boolean>;
  /** Beendet den Prozess */
  kill(pid: number): void;
}

const WAIT_MS = 5000;

// Beendet den laufenden Harness-Server (nach Updates, damit die neue Version startet).
export class ServerStopper {
  private readonly info: ServerInfoFile;
  /** Port des zuletzt beendeten Servers (für einen Neustart) */
  stoppedPort: number | undefined;

  constructor(
    home: HarnessHome,
    private readonly deps: StopDeps = { isOurs: (port) => this.healthIsOurs(port), kill: (pid) => process.kill(pid, 'SIGTERM') },
  ) {
    this.info = new ServerInfoFile(home);
  }

  // Beendet den Server; liefert false, wenn keiner läuft. Veraltete Angaben werden bereinigt.
  async stop(): Promise<boolean> {
    const info = await this.info.read();
    if (!info) return false;
    if (!(await this.deps.isOurs(info.port))) {
      await this.info.remove();
      return false;
    }
    try {
      this.deps.kill(info.pid);
    } catch {
      // Prozess war schon weg; unten wird geprüft, ob der Port noch antwortet
    }
    const deadline = Date.now() + WAIT_MS;
    while (Date.now() < deadline && (await this.deps.isOurs(info.port))) {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    await this.info.remove();
    this.stoppedPort = info.port;
    return true;
  }

  // Fragt den Health-Aufruf; nur ein Server mit dem Namen `ai-harness` zählt
  private async healthIsOurs(port: number): Promise<boolean> {
    try {
      const res = await fetch(`http://127.0.0.1:${port}/api/health`, { signal: AbortSignal.timeout(1500) });
      const body = (await res.json().catch(() => null)) as { name?: string } | null;
      return body?.name === 'ai-harness';
    } catch {
      return false;
    }
  }
}
