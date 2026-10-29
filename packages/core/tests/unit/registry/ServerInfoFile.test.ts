import { spawn, type ChildProcess } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { HarnessHome } from '../../../src/registry/HarnessHome.js';
import { ServerInfoFile } from '../../../src/registry/ServerInfoFile.js';

// Eine Prozessnummer, die nie vergeben ist (wie in den übrigen Prozess-Tests)
const DEAD_PID = 2_147_483_646;

describe('ServerInfoFile', () => {
  let dir: string;
  let file: ServerInfoFile;
  let other: ChildProcess | null = null;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'server-info-'));
    file = new ServerInfoFile(new HarnessHome({ HARNESS_HOME: dir }));
  });
  afterEach(() => {
    other?.kill('SIGKILL');
    other = null;
    rmSync(dir, { recursive: true, force: true });
  });

  // Ein zweiter laufender Prozess, der als fremder Server eingetragen wird
  const runningPid = () => {
    other = spawn(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], { stdio: 'ignore' });
    return other.pid!;
  };

  it('trägt den Server ein, wenn die Datei fehlt oder ein beendeter Server darin steht', async () => {
    expect(await file.claim({ pid: process.pid, port: 4710, startedAt: 'a' })).toBe(true);
    expect(await file.read()).toEqual({ pid: process.pid, port: 4710, startedAt: 'a' });

    await file.write({ pid: DEAD_PID, port: 4711, startedAt: 'b' });
    expect(await file.claim({ pid: process.pid, port: 4712, startedAt: 'c' })).toBe(true);
    expect((await file.read())?.port).toBe(4712);
  });

  it('überschreibt keinen anderen laufenden Server', async () => {
    const pid = runningPid();
    await file.write({ pid, port: 4710, startedAt: 'echt' });

    expect(await file.claim({ pid: process.pid, port: 4719, startedAt: 'test' })).toBe(false);
    expect(await file.read()).toEqual({ pid, port: 4710, startedAt: 'echt' });
  });

  it('löscht beim Beenden nur die eigenen Angaben', async () => {
    const pid = runningPid();
    await file.write({ pid, port: 4710, startedAt: 'echt' });

    await file.release(process.pid);
    expect((await file.read())?.pid).toBe(pid);

    await file.release(pid);
    expect(await file.read()).toBeNull();
  });
});
