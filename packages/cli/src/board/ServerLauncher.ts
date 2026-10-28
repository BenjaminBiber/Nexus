import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import { dirname } from 'node:path';
import { isSea } from 'node:sea';
import { HarnessError, HarnessHome } from '@harness/core';

export type PortState = 'ours' | 'free' | 'foreign';

const WAIT_MS = 10000;

// Erkennt, ob der Harness-Server läuft, und startet ihn bei Bedarf als abgelöstes Hintergrundprogramm.
export class ServerLauncher {
  // sea: läuft die CLI als gebündeltes Sidecar? Dann startet das Programm sich selbst mit „serve“.
  // Tests setzen den Wert selbst.
  constructor(private readonly sea: boolean = isSea()) {}

  // Prüft den Port: eigener Server, frei oder von einem fremden Programm belegt
  async probe(port: number): Promise<PortState> {
    try {
      const res = await fetch(`http://127.0.0.1:${port}/api/health`, { signal: AbortSignal.timeout(2000) });
      const body = (await res.json().catch(() => null)) as { name?: string } | null;
      return body?.name === 'ai-harness' ? 'ours' : 'foreign';
    } catch (error) {
      const code = (error as { cause?: { code?: string } }).cause?.code;
      return code === 'ECONNREFUSED' ? 'free' : 'foreign';
    }
  }

  // Startet den Server und wartet, bis er antwortet
  async start(port: number, waitMs = WAIT_MS): Promise<void> {
    const child = spawn(process.execPath, this.args(port), {
      detached: true,
      stdio: 'ignore',
      windowsHide: true,
      env: { ...process.env, ...this.bundledWebDir() },
    });
    child.unref();
    const deadline = Date.now() + waitMs;
    while (Date.now() < deadline) {
      if ((await this.probe(port)) === 'ours') return;
      await new Promise((resolve) => setTimeout(resolve, 150));
    }
    throw new HarnessError(
      'server-start-failed',
      `Der Server ist nicht gestartet. Details stehen in ${new HarnessHome().logFile}`,
    );
  }

  // Argumente des Starts: im Sidecar startet das Programm sich selbst mit „serve“, sonst Node mit der Programmdatei
  private args(port: number): string[] {
    if (this.sea && !process.env.HARNESS_SERVER_ENTRY) return ['serve', '--port', String(port)];
    return [...process.execArgv, this.entry(), '--port', String(port)];
  }

  // Programmdatei des Servers: Umgebungsvariable (Tests), neben der gebündelten CLI (Plugin) oder das Paket (Entwicklung)
  private entry(): string {
    if (process.env.HARNESS_SERVER_ENTRY) return process.env.HARNESS_SERVER_ENTRY;
    const require = createRequire(import.meta.url);
    try {
      return require.resolve('./server.mjs');
    } catch {
      return require.resolve('@harness/server');
    }
  }

  // Im Plugin liegt die Oberfläche neben `lib/`; im Sidecar findet der Server sie selbst neben dem Programm,
  // sonst bleibt die Einstellung des Servers maßgeblich
  private bundledWebDir(): Record<string, string> {
    if (process.env.HARNESS_WEB_DIR || this.sea) return {};
    try {
      return { HARNESS_WEB_DIR: dirname(createRequire(import.meta.url).resolve('../web/index.html')) };
    } catch {
      return {};
    }
  }
}
