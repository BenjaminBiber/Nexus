import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { ServerHandle } from '../lib/ServerHandle.mjs';

// Startskript wie `scripts/contract/rust-vor-node.mjs`: startet einen Enkel, der SIGTERM ignoriert,
// beantwortet `/api/health` und schreibt die PID des Enkels in eine Datei
const START_SCRIPT = `
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { writeFileSync } from 'node:fs';
const [port, pidFile] = process.argv.slice(2);
const grandchild = spawn(process.execPath, ['-e', "process.on('SIGTERM', () => {}); setInterval(() => {}, 1000);"], { stdio: 'ignore' });
writeFileSync(pidFile, String(grandchild.pid));
createServer((_, response) => response.end('{}')).listen(Number(port), '127.0.0.1');
`;

// Läuft der Prozess noch?
function alive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

describe.skipIf(process.platform === 'win32')('ServerHandle', () => {
  let dir;

  afterEach(() => {
    if (dir) rmSync(dir, { recursive: true, force: true });
  });

  it('beendet beim Stoppen auch Enkelprozesse, die SIGTERM ignorieren', async () => {
    dir = mkdtempSync(join(tmpdir(), 'server-handle-'));
    const script = join(dir, 'start.mjs');
    const pidFile = join(dir, 'enkel.pid');
    writeFileSync(script, START_SCRIPT);
    const port = await ServerHandle.freePort();

    const handle = await ServerHandle.start({
      command: `${process.execPath} ${script} {port} ${pidFile}`,
      port,
      cwd: dir,
      env: process.env,
    });
    const grandchild = Number(readFileSync(pidFile, 'utf8'));
    expect(alive(grandchild)).toBe(true);

    await handle.stop();

    await expect.poll(() => alive(grandchild), { timeout: 2000 }).toBe(false);
  }, 20000);

  it('stoppt ohne Fehler, wenn der Server schon beendet ist', async () => {
    dir = mkdtempSync(join(tmpdir(), 'server-handle-'));
    const script = join(dir, 'start.mjs');
    writeFileSync(script, START_SCRIPT);
    const port = await ServerHandle.freePort();
    const handle = await ServerHandle.start({
      command: `${process.execPath} ${script} {port} ${join(dir, 'enkel.pid')}`,
      port,
      cwd: dir,
      env: process.env,
    });

    await handle.stop();
    await handle.stop();
  }, 20000);
});
