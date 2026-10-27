import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { TokenStore } from '@harness/core';
import type { MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';

describe('FavoriteRoutes', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;
  let id: string;
  let repoPath: string;
  let token: string;
  let cwd: string;
  let base: string;

  // Sendet ein Hook-Ereignis mit dem Token des Projekts
  const hook = (event: object) => server.post('/api/hook', { project: repoPath, event }, { 'x-harness-token': token });
  const start = (sessionId: string, extra: object = {}) => hook({ kind: 'session-start', sessionId, cwd, tickets: ['ARC-0001'], ...extra });

  beforeEach(async () => {
    ({ server, backend } = await TestServer.startWithBoard());
    const added = await server.addProject();
    id = added.id;
    repoPath = added.repo.path;
    token = await new TokenStore(added.project.layout).read();
    await backend.services.projects.create('ARC', 'Arches', backend.actor());
    const board = await backend.services.boards.create('ARC', 'Sprint', backend.actor());
    await backend.services.tickets.create(board.id, { title: 'Erstes' }, backend.actor());
    cwd = mkdtempSync(join(tmpdir(), 'harness fav '));
    base = `/api/projects/${id}/favorites`;
  });
  afterEach(async () => {
    await server.dispose();
    rmSync(cwd, { recursive: true, force: true });
  });

  it('markiert eine bekannte Session mit Notiz und liefert Befehl und Ticketangaben', async () => {
    await start('s1');
    const res = await server.put(`${base}/s1`, { note: 'Weiter mit Tests' });
    expect(res.status).toBe(200);
    expect(res.json).toMatchObject({
      sessionId: 's1', note: 'Weiter mit Tests', tickets: ['ARC-0001'], live: true, cwdExists: true,
      ticketInfo: [{ id: 'ARC-0001', title: 'Erstes' }],
    });
    expect(res.json.command).toBe(`cd "${cwd}" && claude --resume s1`);
    expect((await server.get(base)).json).toHaveLength(1);
  });

  it('meldet eine unbekannte Session mit 422', async () => {
    const res = await server.put(`${base}/gibtsnicht`, {});
    expect(res.status).toBe(422);
    expect(res.json.code).toBe('session-unknown');
  });

  it('ändert die Notiz und entfernt den Favoriten wieder', async () => {
    await start('s1');
    await server.put(`${base}/s1`, { note: 'a' });
    const changed = await server.put(`${base}/s1`, { note: 'b' });
    expect(changed.json.note).toBe('b');
    expect((await server.delete(`${base}/s1`)).status).toBe(204);
    expect((await server.get(base)).json).toEqual([]);
    expect((await server.get(`/api/projects/${id}/sessions`)).json).toHaveLength(1);
  });

  it('trennt Favoriten je Projekt', async () => {
    await start('s1');
    await server.put(`${base}/s1`, {});
    const other = await server.addProject();
    expect((await server.get(`/api/projects/${other.id}/favorites`)).json).toEqual([]);
  });

  it('behält den Favoriten nach einem Neustart des Servers und zeigt den letzten Zustand', async () => {
    await start('s1');
    await server.put(`${base}/s1`, { note: 'bleibt' });
    await hook({ kind: 'user-prompt', sessionId: 's1', cwd });
    const home = server.home;
    const { ServerApp } = await import('../../src/ServerApp.js');
    const again = await ServerApp.create({ port: 4710, host: '127.0.0.1', home, webDir: null, version: 'test' });
    const res = await again.inject({ method: 'GET', url: base, headers: { host: '127.0.0.1:4710' } });
    await again.close();
    const list = JSON.parse(res.body);
    expect(list[0]).toMatchObject({ sessionId: 's1', note: 'bleibt', live: false, state: 'running', tickets: ['ARC-0001'] });
  });

  it('weist ein fehlendes Arbeitsverzeichnis aus und lässt den Befehl bestehen', async () => {
    await start('s1');
    await server.put(`${base}/s1`, {});
    rmSync(cwd, { recursive: true, force: true });
    const [fav] = (await server.get(base)).json;
    expect(existsSync(cwd)).toBe(false);
    expect(fav.cwdExists).toBe(false);
    expect(fav.command).toContain('claude --resume s1');
  });

  it('ordnet eine fortgesetzte Session dem einzigen Favoriten zu', async () => {
    await start('s1');
    await server.put(`${base}/s1`, {});
    await start('s2', { source: 'resume' });
    const [fav] = (await server.get(base)).json;
    expect(fav.resumedAs).toEqual(['s2']);
  });

  it('ordnet einen normalen Start nicht zu', async () => {
    await start('s1');
    await server.put(`${base}/s1`, {});
    await start('s2', { source: 'startup' });
    expect((await server.get(base)).json[0].resumedAs).toEqual([]);
  });
});
