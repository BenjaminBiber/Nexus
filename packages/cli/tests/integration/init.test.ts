import { describe, it, expect, afterEach } from 'vitest';
import { mkdtempSync, rmSync, existsSync, realpathSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { BoardConfigStore, HarnessHome } from '@harness/core';
import { TempProject } from './helpers/TempProject.js';

describe('harness init', () => {
  const projects: TempProject[] = [];
  const make = () => {
    const p = new TempProject();
    projects.push(p);
    return p;
  };
  afterEach(() => projects.splice(0).forEach((p) => p.dispose()));

  it('richtet das Projekt ohne Ticket-Ordner ein und weist auf die fehlende Board-Datenbank hin', () => {
    const p = make();
    const result = p.run(['init']);
    expect(result.stderr).toBe('');
    expect(result.code).toBe(0);
    expect(result.stdout).toContain('Harness eingerichtet');
    expect(result.stdout).toContain('Board-Datenbank ist noch nicht eingerichtet');
    expect(existsSync(join(p.path, '.git', 'harness', 'token'))).toBe(true);
    expect(existsSync(join(p.path, '.git', 'harness', 'tickets'))).toBe(false);
  });

  it('liefert mit --json Ordner, Plugin-Schalter und Board-Hinweis', () => {
    const p = make();
    const json = JSON.parse(p.run(['--json', 'init']).stdout);
    expect(json).toMatchObject({ pluginEnabled: true, board: null });
    expect(json.boardHint).toContain('Board-Datenbank');
  });

  it('ist wiederholbar und schnell', () => {
    const p = make();
    p.run(['init']);
    const start = Date.now();
    expect(p.run(['init']).code).toBe(0);
    expect(Date.now() - start).toBeLessThan(5000);
  });

  it('schreibende Befehle ohne init geben einen Hinweis aus und schreiben nichts', () => {
    const p = make();
    const result = p.run(['spec', 'resolve', 'K-1a2b3c4d', 'Test']);
    expect(result.code).toBe(1);
    expect(result.stderr).toContain('harness init');
    expect(existsSync(join(p.path, '.git', 'harness'))).toBe(false);
  });

  it('init außerhalb eines Git-Projekts meldet das und legt nichts an', () => {
    const dir = realpathSync(mkdtempSync(join(tmpdir(), 'harness-nogit-')));
    try {
      const result = make().run(['init'], dir);
      expect(result.code).toBe(1);
      expect(result.stderr).toContain('kein Git-Projekt');
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  // board.yaml im HARNESS_HOME des Testprojekts mit einem Server, der nicht antwortet
  const unreachable = (p: TempProject, server: string, port: number) =>
    new BoardConfigStore(new HarnessHome({ HARNESS_HOME: join(p.base, 'home') })).write({
      provider: 'mssql', file: join(p.base, 'home', 'board.db'),
      server, port, database: 'AiHarness_Nirgends', user: 'x', password: 'x', encrypt: true, trustServerCertificate: true,
      userId: '11111111-1111-1111-1111-111111111111', userName: 'Test', activeBoard: null,
    });

  it('meldet eine nicht erreichbare Board-Datenbank nur als Hinweis und endet mit 0', async () => {
    const p = make();
    await unreachable(p, '127.0.0.1', 1);
    const result = p.run(['--json', 'init']);
    expect(result.code, result.stderr).toBe(0);
    const json = JSON.parse(result.stdout);
    expect(json).toMatchObject({ board: null });
    expect(json.boardHint).toContain('Board-Datenbank');
  });

  it('wartet höchstens kurz auf die Board-Datenbank (Zeitlimit) und endet mit 0', async () => {
    const p = make();
    // Nicht routbare Adresse: die Verbindung hängt, bis das Zeitlimit greift
    await unreachable(p, '10.255.255.1', 1433);
    const start = Date.now();
    const result = p.run(['init']);
    expect(result.code, result.stderr).toBe(0);
    expect(result.stdout).toContain('Harness eingerichtet');
    expect(result.stdout).toMatch(/Board-Datenbank/);
    expect(Date.now() - start).toBeLessThan(7000);
  }, 20000);
});
