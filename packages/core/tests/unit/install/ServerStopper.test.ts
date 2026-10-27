import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { HarnessHome } from '../../../src/registry/HarnessHome.js';
import { ServerInfoFile } from '../../../src/registry/ServerInfoFile.js';
import { ServerStopper } from '../../../src/install/ServerStopper.js';

describe('ServerStopper', () => {
  let dir: string;
  let home: HarnessHome;
  let info: ServerInfoFile;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'harness-stop-'));
    home = new HarnessHome({ HARNESS_HOME: dir });
    info = new ServerInfoFile(home);
  });

  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it('meldet false, wenn es keine Serverangaben gibt', async () => {
    const kills: number[] = [];
    const stopper = new ServerStopper(home, { isOurs: async () => false, kill: (pid) => void kills.push(pid) });
    expect(await stopper.stop()).toBe(false);
    expect(kills).toEqual([]);
  });

  it('beendet den Prozess aus den Angaben und räumt auf', async () => {
    await info.write({ pid: 4242, port: 4710, startedAt: 'jetzt' });
    let alive = true;
    const kills: number[] = [];
    const stopper = new ServerStopper(home, {
      isOurs: async () => alive,
      kill: (pid) => {
        kills.push(pid);
        alive = false;
      },
    });
    expect(await stopper.stop()).toBe(true);
    expect(kills).toEqual([4242]);
    expect(existsSync(home.serverFile)).toBe(false);
  });

  it('beendet nichts, wenn unter dem Port kein Harness-Server antwortet (veraltete Angaben)', async () => {
    await info.write({ pid: 4242, port: 4710, startedAt: 'jetzt' });
    const kills: number[] = [];
    const stopper = new ServerStopper(home, { isOurs: async () => false, kill: (pid) => void kills.push(pid) });
    expect(await stopper.stop()).toBe(false);
    expect(kills).toEqual([]);
    expect(existsSync(home.serverFile)).toBe(false);
  });
});
