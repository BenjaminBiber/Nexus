import type { WorkerLauncher } from '../../src/dispatch/WorkerLauncher.js';

// Startet keine Prozesse, merkt sich nur die Aufrufe
export class FakeLauncher implements WorkerLauncher {
  readonly launched: Array<{ runId: string; env: NodeJS.ProcessEnv }> = [];
  fail: Error | null = null;

  // Merkt den Aufruf und liefert eine erfundene Prozess-ID
  launch(runId: string, env: NodeJS.ProcessEnv): number {
    if (this.fail) throw this.fail;
    this.launched.push({ runId, env });
    return 4242;
  }
}
