import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { createServer, type Server } from 'node:http';
import { createServer as createNetServer } from 'node:net';
import { existsSync, mkdtempSync, readFileSync, realpathSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { TempProject } from './helpers/TempProject.js';

const SERVER_ENTRY = resolve(__dirname, '../../../server/src/main.ts');

// Sucht einen freien Port
function freePort(): Promise<number> {
  return new Promise((done) => {
    const s = createNetServer();
    s.listen(0, '127.0.0.1', () => {
      const port = (s.address() as { port: number }).port;
      s.close(() => done(port));
    });
  });
}

describe('harness board', () => {
  let p: TempProject;
  let home: string;
  let env: Record<string, string>;
  let foreign: Server | null = null;

  beforeEach(() => {
    p = new TempProject();
    home = realpathSync(mkdtempSync(join(tmpdir(), 'harness-board-')));
    env = { HARNESS_HOME: home, HARNESS_SERVER_ENTRY: SERVER_ENTRY };
  });

  afterEach(async () => {
    // Server beenden, den der Test gestartet hat
    const infoFile = join(home, 'server.json');
    if (existsSync(infoFile)) {
      try {
        process.kill(JSON.parse(readFileSync(infoFile, 'utf8')).pid);
      } catch {
        /* läuft nicht mehr */
      }
    }
    await new Promise<void>((done) => (foreign ? foreign.close(() => done()) : done()));
    foreign = null;
    await new Promise((r) => setTimeout(r, 300));
    p.dispose();
    rmSync(home, { recursive: true, force: true });
  });

  it('startet den Server, meldet das Projekt an und nennt die Adresse', async () => {
    p.run(['init']);
    const port = await freePort();
    const result = p.run(['--json', 'board', '--no-open', '--port', String(port)], p.path, env);
    expect(result.stderr).toBe('');
    expect(result.code).toBe(0);
    const out = JSON.parse(result.stdout);
    expect(out).toMatchObject({ port, started: true });
    expect(out.url).toMatch(new RegExp(`^http://127\\.0\\.0\\.1:${port}/p/[0-9a-f]{8}/board$`));
    const health = await fetch(`http://127.0.0.1:${port}/api/health`);
    expect((await health.json()).name).toBe('ai-harness');
    const projects = await (await fetch(`http://127.0.0.1:${port}/api/projects`)).json();
    expect(projects).toHaveLength(1);
  });

  it('startet bei laufendem Server keinen zweiten und meldet weitere Projekte an', async () => {
    p.run(['init']);
    const port = await freePort();
    const first = JSON.parse(p.run(['--json', 'board', '--no-open', '--port', String(port)], p.path, env).stdout);
    const second = new TempProject();
    try {
      second.run(['init']);
      const result = second.run(['--json', 'board', '--no-open', '--port', String(port)], second.path, env);
      expect(result.code).toBe(0);
      const out = JSON.parse(result.stdout);
      expect(out.started).toBe(false);
      expect(out.url).not.toBe(first.url);
      const projects = await (await fetch(`http://127.0.0.1:${port}/api/projects`)).json();
      expect(projects).toHaveLength(2);
    } finally {
      second.dispose();
    }
  });

  it('gibt ohne --json zwei lesbare Zeilen aus', async () => {
    p.run(['init']);
    const port = await freePort();
    const result = p.run(['board', '--no-open', '--port', String(port)], p.path, env);
    expect(result.stdout).toContain(`Server läuft auf http://127.0.0.1:${port}`);
    expect(result.stdout).toMatch(/Board: http:\/\/127\.0\.0\.1:\d+\/p\/[0-9a-f]{8}\/board/);
  });

  it('meldet einen Port, den ein fremdes Programm belegt, verständlich', async () => {
    p.run(['init']);
    const port = await freePort();
    foreign = createServer((_req, res) => res.end('ich bin etwas anderes'));
    await new Promise<void>((done) => foreign!.listen(port, '127.0.0.1', done));
    const result = p.run(['board', '--no-open', '--port', String(port)], p.path, env);
    expect(result.code).toBe(1);
    expect(result.stderr).toContain(`Port ${port} wird von einem anderen Programm benutzt`);
    expect(result.stderr).toContain('--port');
  });

  it('verlangt ein eingerichtetes Projekt', async () => {
    const port = await freePort();
    const result = p.run(['board', '--no-open', '--port', String(port)], p.path, env);
    expect(result.code).toBe(1);
    expect(result.stderr).toContain('harness init');
    expect(existsSync(join(home, 'server.json'))).toBe(false);
  });
});
