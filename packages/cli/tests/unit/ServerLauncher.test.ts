import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const spawn = vi.hoisted(() => vi.fn());
vi.mock('node:child_process', () => ({ spawn }));

const { ServerLauncher } = await import('../../src/board/ServerLauncher.js');

const PORT = 45710;

// Start des Servers: ohne Sidecar startet die CLI Node mit der Programmdatei, im Sidecar das Programm selbst mit „serve“.
describe('ServerLauncher', () => {
  beforeEach(() => {
    spawn.mockReset();
    spawn.mockReturnValue({ unref: () => undefined });
    // Der Server „antwortet“ sofort, damit start() nicht wartet
    vi.stubGlobal('fetch', vi.fn(async () => ({ json: async () => ({ name: 'ai-harness' }) })));
    process.env.HARNESS_SERVER_ENTRY = '/pfad/server.mjs';
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    delete process.env.HARNESS_SERVER_ENTRY;
    delete process.env.HARNESS_WEB_DIR;
  });

  it('startet Node mit der Programmdatei des Servers', async () => {
    await new ServerLauncher(false).start(PORT);
    expect(spawn.mock.calls[0][0]).toBe(process.execPath);
    expect(spawn.mock.calls[0][1]).toEqual([...process.execArgv, '/pfad/server.mjs', '--port', String(PORT)]);
    expect(spawn.mock.calls[0][2]).toMatchObject({ detached: true, stdio: 'ignore', windowsHide: true });
  });

  it('startet im Sidecar das Programm selbst mit „serve“', async () => {
    delete process.env.HARNESS_SERVER_ENTRY;
    await new ServerLauncher(true).start(PORT);
    expect(spawn.mock.calls[0][0]).toBe(process.execPath);
    expect(spawn.mock.calls[0][1]).toEqual(['serve', '--port', String(PORT)]);
  });

  it('nimmt auch im Sidecar HARNESS_SERVER_ENTRY vor, wenn es gesetzt ist', async () => {
    await new ServerLauncher(true).start(PORT);
    expect(spawn.mock.calls[0][1]).toEqual([...process.execArgv, '/pfad/server.mjs', '--port', String(PORT)]);
  });

  it('lässt ein gesetztes HARNESS_WEB_DIR unangetastet', async () => {
    process.env.HARNESS_WEB_DIR = '/wo/web';
    await new ServerLauncher(true).start(PORT);
    expect(spawn.mock.calls[0][2].env.HARNESS_WEB_DIR).toBe('/wo/web');
  });

  it('setzt im Sidecar kein HARNESS_WEB_DIR: dort findet der Server die Oberfläche selbst', async () => {
    await new ServerLauncher(true).start(PORT);
    expect(spawn.mock.calls[0][2].env.HARNESS_WEB_DIR).toBeUndefined();
  });

  it('meldet einen Fehler, wenn der Server nicht antwortet', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => { throw new TypeError('fetch failed', { cause: { code: 'ECONNREFUSED' } }); }));
    await expect(new ServerLauncher(false).start(PORT, 300)).rejects.toMatchObject({ code: 'server-start-failed' });
  });

  it('unterscheidet eigenen Server, freien Port und fremdes Programm', async () => {
    const launcher = new ServerLauncher(false);
    expect(await launcher.probe(PORT)).toBe('ours');
    vi.stubGlobal('fetch', vi.fn(async () => ({ json: async () => ({ name: 'etwas-anderes' }) })));
    expect(await launcher.probe(PORT)).toBe('foreign');
    vi.stubGlobal('fetch', vi.fn(async () => { throw new TypeError('fetch failed', { cause: { code: 'ECONNREFUSED' } }); }));
    expect(await launcher.probe(PORT)).toBe('free');
  });
});
