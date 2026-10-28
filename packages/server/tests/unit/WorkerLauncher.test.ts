import { beforeEach, describe, expect, it, vi } from 'vitest';

const spawn = vi.hoisted(() => vi.fn());
vi.mock('node:child_process', () => ({ spawn }));

const { ProcessWorkerLauncher } = await import('../../src/dispatch/WorkerLauncher.js');

// Argumente des Worker-Starts: ohne Sidecar braucht es die Programmdatei, im Sidecar ist das Programm selbst der Einstieg.
describe('ProcessWorkerLauncher', () => {
  beforeEach(() => {
    spawn.mockReset();
    spawn.mockReturnValue({ pid: 4711, on: () => undefined, unref: () => undefined });
  });

  it('startet Node mit der Programmdatei und --dispatch-worker', () => {
    const pid = new ProcessWorkerLauncher('/pfad/server.mjs', [], false).launch('ARC-0001-x', { A: '1' });
    expect(pid).toBe(4711);
    expect(spawn).toHaveBeenCalledWith(process.execPath, ['/pfad/server.mjs', '--dispatch-worker', 'ARC-0001-x'], {
      detached: true, stdio: 'ignore', windowsHide: true, env: { A: '1' },
    });
  });

  it('gibt die Schalter des Servers mit, aber keine Debug-Schalter', () => {
    new ProcessWorkerLauncher('/pfad/server.mjs', ['--no-warnings', '--inspect-brk=0', '--debug'], false).launch('ARC-0001-x', {});
    expect(spawn.mock.calls[0][1]).toEqual(['--no-warnings', '/pfad/server.mjs', '--dispatch-worker', 'ARC-0001-x']);
  });

  it('lässt im Sidecar Programmdatei und Schalter weg: das Programm ist der Einstieg', () => {
    new ProcessWorkerLauncher('/pfad/server.mjs', ['--no-warnings'], true).launch('ARC-0001-x', {});
    expect(spawn.mock.calls[0][0]).toBe(process.execPath);
    expect(spawn.mock.calls[0][1]).toEqual(['--dispatch-worker', 'ARC-0001-x']);
  });

  it('meldet null, wenn der Start keine Prozess-ID liefert', () => {
    spawn.mockReturnValue({ pid: undefined, on: () => undefined, unref: () => undefined });
    expect(new ProcessWorkerLauncher('/pfad/server.mjs', [], false).launch('ARC-0001-x', {})).toBeNull();
  });
});
