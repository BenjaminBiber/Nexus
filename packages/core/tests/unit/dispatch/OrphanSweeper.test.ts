import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { OrphanSweeper, type ProcessInfo, type ProcessSystem } from '../../../src/dispatch/OrphanSweeper.js';

const ROOT = '/repo-worktrees/ARC-0001';

// Prozessliste: Server (pid 10), Worker (11, Kind des Servers), Shell des Nutzers (20)
const base: ProcessInfo[] = [
  { pid: 1, ppid: 0, command: '/sbin/launchd' },
  { pid: 10, ppid: 1, command: 'node /home/.harness/app/lib/server.mjs --port 4711' },
  { pid: 11, ppid: 10, command: 'node /home/.harness/app/lib/server.mjs --dispatch-worker' },
  { pid: 20, ppid: 1, command: '/bin/zsh -l' },
];

// Prozesssystem zum Zusehen: merkt sich Signale, Prozesse sterben auf SIGTERM, außer sie stehen in `stubborn`
function fakeSystem(processes: ProcessInfo[], cwds: Record<number, string> = {}, stubborn: number[] = []): ProcessSystem & { signals: string[] } {
  const dead = new Set<number>();
  const signals: string[] = [];
  return {
    signals,
    list: async () => processes,
    cwds: async (pids) => new Map(pids.filter((pid) => cwds[pid]).map((pid) => [pid, cwds[pid]])),
    signal(pid, signal) {
      signals.push(`${signal} ${pid}`);
      if (signal === 'SIGKILL' || !stubborn.includes(pid)) dead.add(pid);
    },
    alive: (pid) => !dead.has(pid),
    sleep: async () => undefined,
  };
}

describe('OrphanSweeper.select', () => {
  it('wählt verwaiste Prozesse mit dem Worktree in der Befehlszeile samt ihren Kindern', () => {
    const processes = [
      ...base,
      { pid: 30, ppid: 1, command: `${ROOT}/target/debug/nexus serve --port 4710 --node-sidecar ${ROOT}/packages/server/dist/main.js` },
      { pid: 31, ppid: 30, command: 'node main.js --port 0' },
    ];
    expect(OrphanSweeper.select(processes, ROOT, 11).sort()).toEqual([30, 31]);
  });

  it('wählt verwaiste Prozesse, deren Arbeitsordner im Worktree liegt', () => {
    const processes = [...base, { pid: 40, ppid: 1, command: 'node packages/server/dist/main.js', cwd: `${ROOT}/packages` }];
    expect(OrphanSweeper.select(processes, ROOT, 11)).toEqual([40]);
  });

  it('lässt Prozesse mit lebendem Elternprozess stehen, auch wenn sie im Worktree laufen', () => {
    const processes = [...base, { pid: 50, ppid: 20, command: `cargo test --manifest-path ${ROOT}/Cargo.toml`, cwd: ROOT }];
    expect(OrphanSweeper.select(processes, ROOT, 11)).toEqual([]);
  });

  it('verwechselt keinen Worktree mit gleichem Anfang', () => {
    const processes = [...base, { pid: 60, ppid: 1, command: `${ROOT}0/target/debug/nexus serve`, cwd: `${ROOT}0` }];
    expect(OrphanSweeper.select(processes, ROOT, 11)).toEqual([]);
  });

  it('beendet nie sich selbst oder seine Vorfahren', () => {
    const processes = [
      { pid: 1, ppid: 0, command: '/sbin/launchd' },
      { pid: 70, ppid: 1, command: 'node server.mjs --dispatch-worker', cwd: ROOT },
      { pid: 71, ppid: 70, command: 'node sweep', cwd: ROOT },
    ];
    expect(OrphanSweeper.select(processes, ROOT, 71)).toEqual([]);
  });

  it('gilt unter Windows als verwaist, wenn der Elternprozess fehlt', () => {
    const processes = [
      { pid: 4, ppid: 0, command: 'System' },
      { pid: 80, ppid: 7777, command: `C:\\wt\\ARC-0001\\target\\debug\\nexus.exe serve` },
    ];
    expect(OrphanSweeper.select(processes, 'C:\\wt\\ARC-0001', 4)).toEqual([80]);
  });
});

describe('OrphanSweeper.sweep', () => {
  it('schickt erst SIGTERM und nur Zähen danach SIGKILL', async () => {
    const processes = [
      ...base,
      { pid: 30, ppid: 1, command: `${ROOT}/target/debug/nexus serve` },
      { pid: 31, ppid: 1, command: `node ${ROOT}/scripts/dev.mjs` },
    ];
    const system = fakeSystem(processes, {}, [31]);

    const pids = await new OrphanSweeper(system).sweep(ROOT, 11);

    expect(pids.sort()).toEqual([30, 31]);
    expect(system.signals).toEqual(['SIGTERM 30', 'SIGTERM 31', 'SIGKILL 31']);
  });

  it('findet Waisen über den Arbeitsordner, den das System liefert', async () => {
    const processes = [...base, { pid: 40, ppid: 1, command: 'node main.js' }];
    const system = fakeSystem(processes, { 40: ROOT });

    expect(await new OrphanSweeper(system).sweep(ROOT, 11)).toEqual([40]);
  });

  it('tut nichts ohne Waisen aus dem Worktree', async () => {
    const system = fakeSystem(base);
    expect(await new OrphanSweeper(system).sweep(ROOT, 11)).toEqual([]);
    expect(system.signals).toEqual([]);
  });
});

describe('OrphanSweeper.linkedWorktree', () => {
  let dir: string;

  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it('erkennt einen verknüpften Worktree an der Datei .git und lässt den Hauptordner aus', async () => {
    dir = mkdtempSync(join(tmpdir(), 'orphan-sweeper-'));
    const linked = join(dir, 'linked');
    const main = join(dir, 'main');
    mkdirSync(linked);
    mkdirSync(join(main, '.git'), { recursive: true });
    writeFileSync(join(linked, '.git'), 'gitdir: /x/.git/worktrees/linked\n');

    expect(await OrphanSweeper.linkedWorktree(linked)).toBe(true);
    expect(await OrphanSweeper.linkedWorktree(main)).toBe(false);
    expect(await OrphanSweeper.linkedWorktree(join(dir, 'fehlt'))).toBe(false);
  });
});
