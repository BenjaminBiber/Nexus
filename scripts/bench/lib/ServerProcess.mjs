import { spawn, spawnSync } from 'node:child_process';
import { existsSync, readFileSync, rmSync } from 'node:fs';
import { createServer } from 'node:net';
import { join } from 'node:path';

// Zeit, die ein Start höchstens braucht, bis `/api/health` antwortet
const START_TIMEOUT_MS = 60000;
// Zeit, die das Beenden höchstens braucht
const STOP_TIMEOUT_MS = 5000;

// Wartet eine Weile
const delay = (ms) => new Promise((done) => setTimeout(done, ms));

// Startet und beendet den zu messenden Server und wartet auf seine Antworten.
// Der Startbefehl ist austauschbar (`{port}` wird ersetzt), damit später auch ein anderer Server gemessen werden kann.
export class ServerProcess {
  constructor({ cmd, home, claudeConfigDir, pollMs = 10, startTimeoutMs = START_TIMEOUT_MS }) {
    this.cmdTemplate = cmd;
    this.home = home;
    this.claudeConfigDir = claudeConfigDir ?? join(home, 'claude');
    this.pollMs = pollMs;
    this.startTimeoutMs = startTimeoutMs;
    this.child = null;
    this.port = 0;
    this.startedAt = 0;
    this.exit = null;
    this.output = '';
  }

  // Freier Port auf 127.0.0.1 (wie `freePort` in vitest.setup.ts): nie der Standardport des echten Servers
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

  // Adresse des laufenden Servers
  get url() {
    return `http://127.0.0.1:${this.port}`;
  }

  // Pfad der Prozessangaben, die der Server nach `listen` schreibt
  get serverFile() {
    return join(this.home, 'server.json');
  }

  // Startet den Server auf einem freien Port; der Zeitpunkt des Starts ist der Nullpunkt aller Startmessungen
  async start() {
    if (this.child) throw new Error('Der Server läuft schon.');
    this.port = await ServerProcess.freePort();
    const env = {
      ...process.env,
      HARNESS_HOME: this.home,
      HARNESS_PORT: String(this.port),
      CLAUDE_CONFIG_DIR: this.claudeConfigDir,
    };
    // Eine Zuordnung aus der laufenden Claude-Session gilt für die Messung nicht
    delete env.HARNESS_SESSION;
    delete env.HARNESS_TICKET;
    this.exit = null;
    this.output = '';
    this.startedAt = performance.now();
    this.child = spawn(this.cmdTemplate.replaceAll('{port}', String(this.port)), {
      shell: true,
      windowsHide: true,
      env,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    const collect = (chunk) => {
      this.output = `${this.output}${chunk}`.slice(-4000);
    };
    this.child.stdout.on('data', collect);
    this.child.stderr.on('data', collect);
    this.child.on('error', (error) => collect(`\n${error.message}`));
    this.child.on('exit', (code, signal) => {
      this.exit = { code, signal };
    });
  }

  // Wartet auf das erste `200` eines Pfades und liefert die Zeit seit dem Start in Millisekunden
  async waitFor(path) {
    const deadline = performance.now() + this.startTimeoutMs;
    for (;;) {
      if (await this.probe(path)) return performance.now() - this.startedAt;
      if (this.exit) throw new Error(this.failure(`ist beendet (Code ${this.exit.code}, Signal ${this.exit.signal})`));
      if (performance.now() > deadline) throw new Error(this.failure(`antwortet nach ${this.startTimeoutMs} ms nicht auf ${path}`));
      await delay(this.pollMs);
    }
  }

  // Eine einzelne Anfrage: `true`, wenn sie mit 200 antwortet (der Körper wird gelesen, damit die Verbindung frei wird)
  async probe(path) {
    try {
      const res = await fetch(`${this.url}${path}`, { signal: AbortSignal.timeout(5000) });
      await res.arrayBuffer();
      return res.status === 200;
    } catch {
      return false;
    }
  }

  // Misst eine vollständige Anfrage samt Antwortkörper; ein anderer Status als 200 ist ein Fehler
  async measure(path) {
    const begin = performance.now();
    const res = await fetch(`${this.url}${path}`, { signal: AbortSignal.timeout(30000) });
    const body = await res.text();
    const took = performance.now() - begin;
    if (res.status !== 200) throw new Error(`GET ${path} antwortet mit ${res.status}: ${body.slice(0, 300)}`);
    return took;
  }

  // Prozessnummer des Servers: aus `server.json`, sonst aus der Antwort von `/api/health`.
  // Mit `shell: true` ist die Nummer des Kindprozesses die der Shell und taugt nicht zum Messen des Speichers.
  async serverPid() {
    const fromFile = this.pidFromFile();
    if (fromFile !== null) return fromFile;
    try {
      const res = await fetch(`${this.url}/api/health`, { signal: AbortSignal.timeout(5000) });
      const body = await res.json();
      return Number.isInteger(body?.pid) ? body.pid : null;
    } catch {
      return null;
    }
  }

  // Prozessnummer aus `server.json` oder null
  pidFromFile() {
    try {
      const info = JSON.parse(readFileSync(this.serverFile, 'utf8'));
      return Number.isInteger(info?.pid) ? info.pid : null;
    } catch {
      return null;
    }
  }

  // Beendet den Server und wartet, bis `server.json` weg ist (höchstens 5 s)
  async stop(waitMs = STOP_TIMEOUT_MS) {
    const child = this.child;
    if (!child) return;
    const serverPid = this.pidFromFile();
    if (process.platform === 'win32') {
      // Die Shell und ihre Kinder hart beenden; ein freundliches Signal gibt es dort nicht
      spawnSync('taskkill', ['/pid', String(child.pid), '/T', '/F'], { windowsHide: true });
    } else {
      // Server und Shell können derselbe Prozess sein (sh ersetzt sich meist selbst); doppeltes Signal stört nicht
      for (const pid of new Set([serverPid, child.pid].filter((pid) => Number.isInteger(pid) && pid > 0))) {
        try {
          process.kill(pid, 'SIGTERM');
        } catch {
          /* schon beendet */
        }
      }
    }
    const deadline = Date.now() + waitMs;
    while (Date.now() < deadline) {
      if (this.exit && !existsSync(this.serverFile)) break;
      await delay(25);
    }
    if (!this.exit) {
      try {
        child.kill('SIGKILL');
      } catch {
        /* schon beendet */
      }
    }
    // Nach einem harten Ende bleibt die Datei liegen; der nächste Start soll keine fremde Prozessnummer vorfinden
    rmSync(this.serverFile, { force: true });
    this.child = null;
  }

  // Fehlermeldung mit Startbefehl, Ausgabe und Ende des Serverprotokolls
  failure(what) {
    const parts = [`Der Server (${this.cmdTemplate}) ${what}.`];
    if (this.output.trim()) parts.push(`Ausgabe: ${this.output.trim()}`);
    const log = this.logTail();
    if (log) parts.push(`Protokoll: ${log}`);
    return parts.join('\n');
  }

  // Letzte Zeilen von `server.log` im Messordner
  logTail(lines = 10) {
    try {
      return readFileSync(join(this.home, 'server.log'), 'utf8').trimEnd().split('\n').slice(-lines).join('\n');
    } catch {
      return '';
    }
  }
}
