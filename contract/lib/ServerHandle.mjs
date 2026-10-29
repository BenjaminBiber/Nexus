import { spawn, spawnSync } from 'node:child_process';
import { createServer } from 'node:net';
import { isAbsolute, join } from 'node:path';

const READY_TIMEOUT_MS = 15000;
const READY_POLL_MS = 150;
const STOP_TIMEOUT_MS = 5000;

// Startet den Vertragsserver als eigenen Prozess, wartet auf `GET /api/health` und beendet ihn wieder.
// Mit `--base` läuft der Server fremd; dann hält diese Klasse nur die Adresse.
export class ServerHandle {
  constructor(options) {
    this.base = options.base;
    this.child = options.child ?? null;
    this.output = options.output ?? (() => '');
  }

  // Freier Port auf 127.0.0.1
  static freePort() {
    return new Promise((done, fail) => {
      const probe = createServer();
      probe.on('error', fail);
      probe.listen(0, '127.0.0.1', () => {
        const { port } = probe.address();
        probe.close(() => done(port));
      });
    });
  }

  // Zerlegt einen Befehl aus `--server-cmd` bzw. `--cli-cmd` in Programm und Argumente (einfache und doppelte Anführungszeichen)
  static split(command) {
    const parts = command.match(/"[^"]*"|'[^']*'|\S+/g) ?? [];
    return parts.map((part) => (/^["'].*["']$/.test(part) ? part.slice(1, -1) : part));
  }

  // Macht relative Pfade eines Befehls absolut; sie gelten ab der Projektwurzel, auch wenn der Befehl
  // später in einem anderen Ordner läuft (die CLI läuft im Repo des Fixtures)
  static resolve(parts, root) {
    return parts.map((part) => (!isAbsolute(part) && /[\\/]/.test(part) ? join(root, part) : part));
  }

  // Fremder Server: keine Prozesssteuerung, nur die Adresse
  static external(base) {
    return new ServerHandle({ base: base.replace(/\/$/, '') });
  }

  // Startet den Server und wartet, bis er antwortet
  static async start(options) {
    const [command, ...args] = ServerHandle.split(options.command).map((part) => part.replace('{port}', String(options.port)));
    // Unter Unix in eigener Prozessgruppe: `stop` beendet so auch Enkel (Rust-Server, Node-Sidecar),
    // die ein Startskript wie `scripts/contract/rust-vor-node.mjs` sonst verwaist zurücklässt
    const child = spawn(command, args, {
      cwd: options.cwd,
      env: options.env,
      stdio: ['ignore', 'pipe', 'pipe'],
      windowsHide: true,
      detached: process.platform !== 'win32',
    });
    const lines = [];
    const collect = (chunk) => {
      lines.push(chunk.toString());
      if (options.verbose) process.stderr.write(chunk);
    };
    child.stdout.on('data', collect);
    child.stderr.on('data', collect);
    let exit = null;
    child.on('exit', (code, signal) => {
      exit = { code, signal };
    });
    const handle = new ServerHandle({ base: `http://127.0.0.1:${options.port}`, child, output: () => lines.join('').trim() });
    try {
      await handle.waitForHealth(() => exit);
    } catch (error) {
      await handle.stop();
      throw error;
    }
    return handle;
  }

  // Wartet höchstens 15 s auf eine Antwort von `GET /api/health`
  async waitForHealth(exitOf = () => null) {
    const until = Date.now() + READY_TIMEOUT_MS;
    let last = '';
    while (Date.now() < until) {
      const exit = exitOf();
      if (exit) {
        throw new Error(`Der Server hat sich beendet (Code ${exit.code}, Signal ${exit.signal}):\n${this.output()}`);
      }
      try {
        const response = await fetch(`${this.base}/api/health`);
        if (response.ok) {
          await response.arrayBuffer();
          return;
        }
        last = `HTTP ${response.status}`;
      } catch (error) {
        last = error.message;
      }
      await new Promise((done) => setTimeout(done, READY_POLL_MS));
    }
    throw new Error(`Der Server unter ${this.base} antwortet nicht (${last}).\n${this.output()}`);
  }

  // Beendet den Server samt allen Kindprozessen (unter Unix die ganze Prozessgruppe)
  async stop() {
    const child = this.child;
    if (!child) return;
    if (process.platform === 'win32') {
      if (child.exitCode !== null || child.signalCode !== null) return;
      const done = new Promise((resolve) => child.once('exit', resolve));
      spawnSync('taskkill', ['/T', '/F', '/PID', String(child.pid)], { windowsHide: true, stdio: 'ignore' });
      await done;
      return;
    }
    const running = child.exitCode === null && child.signalCode === null;
    const done = running ? new Promise((resolve) => child.once('exit', resolve)) : Promise.resolve();
    ServerHandle.signalGroup(child.pid, 'SIGTERM');
    if (!(await ServerHandle.groupGone(child.pid, STOP_TIMEOUT_MS))) ServerHandle.signalGroup(child.pid, 'SIGKILL');
    await done;
  }

  // Schickt allen Prozessen der Gruppe ein Signal; eine schon leere Gruppe ist kein Fehler
  static signalGroup(pid, signal) {
    try {
      process.kill(-pid, signal);
    } catch {
      // Gruppe gibt es nicht mehr
    }
  }

  // Wartet höchstens `timeoutMs`, bis kein Prozess der Gruppe mehr läuft; `true`, wenn sie leer ist
  static async groupGone(pid, timeoutMs) {
    const until = Date.now() + timeoutMs;
    while (Date.now() < until) {
      try {
        process.kill(-pid, 0);
      } catch {
        return true;
      }
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    return false;
  }
}
