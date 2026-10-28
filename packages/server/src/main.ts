import { appendFileSync, mkdirSync, statSync, truncateSync } from 'node:fs';
import { HarnessError, HarnessHome, ServerInfoFile } from '@harness/core';
import { SqliteWarningFilter } from '@harness/db';
import { DispatchWorker } from './dispatch/DispatchWorker.js';
import { ServerApp } from './ServerApp.js';
import { ServerConfigLoader } from './config/ServerConfigLoader.js';

const MAX_LOG_BYTES = 1024 * 1024;

// Startpunkt des Servers: Port öffnen, Prozessangaben schreiben, sauber beenden.
async function main(): Promise<void> {
  SqliteWarningFilter.install();
  const config = new ServerConfigLoader().load(process.argv.slice(2));
  const log = (line: string) => writeLog(config.home.dir, config.home.logFile, line);
  const app = await ServerApp.create({ ...config, log });
  try {
    await app.listen({ port: config.port, host: config.host });
  } catch (error) {
    const busy = (error as NodeJS.ErrnoException).code === 'EADDRINUSE';
    log(busy ? `Port ${config.port} ist belegt.` : `Start fehlgeschlagen: ${(error as Error).message}`);
    console.error(busy ? `Port ${config.port} ist belegt.` : (error as Error).message);
    process.exit(1);
  }
  const port = (app.server.address() as { port: number }).port;
  app.guard.allowPort(port);
  // Als Sidecar gehört `server.json` dem Rust-Server davor; dieser Prozess meldet nur seinen Port
  const info = config.sidecar ? null : new ServerInfoFile(config.home);
  await info?.write({ pid: process.pid, port, startedAt: new Date().toISOString() });
  log(`Server läuft auf http://${config.host}:${port}`);

  const stop = async () => {
    log('Server wird beendet.');
    await app.close();
    await info?.remove();
    process.exit(0);
  };
  process.on('SIGINT', stop);
  process.on('SIGTERM', stop);
  if (config.sidecar) announcePort(port, stop);
}

// Sidecar-Modus: Der Rust-Server davor liest den gewählten Port von stdout. Solange seine Pipe auf
// stdin offen ist, lebt er; endet sie, beendet sich dieser Prozess selbst (gilt auf allen Plattformen).
function announcePort(port: number, stop: () => Promise<void>): void {
  process.stdout.write(`HARNESS_SIDECAR_PORT=${port}\n`);
  process.stdin.on('end', stop);
  process.stdin.on('close', stop);
  process.stdin.on('error', stop);
  process.stdin.resume();
}

// Hängt eine Zeile an das Protokoll an; über 1 MB wird neu begonnen
function writeLog(dir: string, file: string, line: string): void {
  try {
    mkdirSync(dir, { recursive: true });
    const size = statSync(file, { throwIfNoEntry: false })?.size ?? 0;
    if (size > MAX_LOG_BYTES) truncateSync(file, 0);
    appendFileSync(file, `${new Date().toISOString()} ${line}\n`);
  } catch {
    /* Protokoll ist nur Hilfe, kein Grund zum Abbruch */
  }
}

// Mit --dispatch-worker <lauf> läuft statt des Servers ein Worker des Dispatchers (losgelöst gestartet)
const workerAt = process.argv.indexOf('--dispatch-worker');
const run = workerAt >= 0 ? new DispatchWorker(new HarnessHome()).run(process.argv[workerAt + 1] ?? '') : main();
run.then(
  () => {
    if (workerAt >= 0) process.exit(0);
  },
  (error) => {
    console.error(error instanceof HarnessError ? error.message : error);
    process.exit(1);
  },
);
