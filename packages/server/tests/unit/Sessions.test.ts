import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { readFileSync, existsSync } from 'node:fs';
import { TokenStore } from '@harness/core';
import { ServerApp } from '../../src/ServerApp.js';
import { LiveServer } from '../helpers/LiveServer.js';
import { PORT, TestServer } from '../helpers/TestServer.js';

describe('Hook-Schnittstelle und Sessions', () => {
  let server: TestServer;
  let id: string;
  let repoPath: string;
  let token: string;

  // Sendet ein Hook-Ereignis mit dem Token des Projekts
  const hook = (event: object, options: { token?: string | null; project?: string } = {}) =>
    server.post(
      '/api/hook',
      { project: options.project ?? repoPath, event },
      options.token === null ? {} : { 'x-harness-token': options.token ?? token },
    );
  const start = { kind: 'session-start', sessionId: 's1', cwd: 'D:/wt', tickets: ['T-0001'] };

  beforeEach(async () => {
    server = await TestServer.start();
    const added = await server.addProject();
    id = added.id;
    repoPath = added.repo.path;
    token = await new TokenStore(added.project.layout).read();
  });
  afterEach(() => server.dispose());

  it('nimmt ein Ereignis mit richtigem Token an und zeigt die Session', async () => {
    const res = await hook(start);
    expect(res.status).toBe(204);
    const list = (await server.get(`/api/projects/${id}/sessions`)).json;
    expect(list).toHaveLength(1);
    expect(list[0]).toMatchObject({
      id: 's1', projectId: id, tickets: ['T-0001'], cwd: 'D:/wt', state: 'idle', displayState: 'idle', listening: true, endedAt: null,
    });
    expect(typeof list[0].runtimeMs).toBe('number');
  });

  it('führt Zustände und Subagenten', async () => {
    await hook(start);
    await hook({ kind: 'user-prompt', sessionId: 's1', cwd: 'D:/wt' });
    await hook({ kind: 'subagent-start', sessionId: 's1', cwd: 'D:/wt', agentId: 'a1', agentType: 'Explore' });
    let session = (await server.get(`/api/projects/${id}/sessions`)).json[0];
    expect(session.state).toBe('running');
    expect(session.subagents).toMatchObject([{ id: 'a1', type: 'Explore', state: 'running', endedAt: null }]);
    await hook({ kind: 'subagent-stop', sessionId: 's1', cwd: 'D:/wt', agentId: 'a1', agentType: 'Explore' });
    await hook({ kind: 'session-end', sessionId: 's1', cwd: 'D:/wt' });
    session = (await server.get(`/api/projects/${id}/sessions`)).json[0];
    expect(session).toMatchObject({ state: 'ended', displayState: 'ended', listening: false });
    expect(session.endedAt).not.toBeNull();
    expect(session.subagents[0].state).toBe('ended');
  });

  it('lehnt fehlendes oder falsches Token mit 403 ab und ändert nichts', async () => {
    expect((await hook(start, { token: null })).status).toBe(403);
    const wrong = await hook(start, { token: 'falsch' });
    expect(wrong.status).toBe(403);
    expect(wrong.json.code).toBe('token-invalid');
    expect((await hook(start, { token: token.slice(0, -1) + (token.endsWith('0') ? '1' : '0') })).status).toBe(403);
    expect((await server.get(`/api/projects/${id}/sessions`)).json).toEqual([]);
  });

  it('meldet für ein nicht angemeldetes oder nicht eingerichtetes Projekt verständliche Fehler', async () => {
    const other = await server.addProject();
    const stranger = other.repo.path;
    // eingerichtet, aber nicht in der Projektliste: Pfad eines frischen, nicht angemeldeten Projekts
    const { HarnessProject } = await import('@harness/core');
    const { TempRepo } = await import('../../../core/tests/unit/helpers/TempRepo.js');
    const fresh = new TempRepo();
    server.repos.push(fresh);
    await HarnessProject.init(fresh.path);
    expect((await hook(start, { project: fresh.path })).status).toBe(404);
    expect(stranger).toBeTruthy();
    const bare = new TempRepo();
    server.repos.push(bare);
    expect((await hook(start, { project: bare.path })).status).toBe(422);
    expect((await hook(start, { project: 'relativ' })).status).toBe(422);
  });

  it('lehnt ungültige Ereignisse mit 422 ab', async () => {
    for (const event of [{}, { kind: 'quatsch', sessionId: 's', cwd: 'x' }, { kind: 'stop', cwd: 'x' }, { kind: 'stop', sessionId: '', cwd: 'x' }, 'text', null]) {
      const res = await hook(event as object);
      expect(res.status, JSON.stringify(event)).toBe(422);
      expect(res.json.code).toBe('event-invalid');
    }
  });

  it('hält die Sessions der Projekte getrennt', async () => {
    const second = await server.addProject();
    const tokenB = await new TokenStore(second.project.layout).read();
    await hook(start);
    await server.post('/api/hook', { project: second.repo.path, event: { ...start, sessionId: 's2' } }, { 'x-harness-token': tokenB });
    expect((await server.get(`/api/projects/${id}/sessions`)).json.map((s: any) => s.id)).toEqual(['s1']);
    expect((await server.get(`/api/projects/${second.id}/sessions`)).json.map((s: any) => s.id)).toEqual(['s2']);
    // Token eines anderen Projekts gilt nicht
    expect((await hook(start, { project: second.repo.path })).status).toBe(403);
  });

  it('speichert die Sessions und lädt sie beim Neustart wieder', async () => {
    await hook(start);
    await hook({ kind: 'subagent-start', sessionId: 's1', cwd: 'D:/wt', agentId: 'a1', agentType: 'Explore' });
    await server.app.close(); // schreibt ausstehende Änderungen
    expect(existsSync(server.home.sessionsFile)).toBe(true);
    expect(JSON.parse(readFileSync(server.home.sessionsFile, 'utf8')).sessions[0].id).toBe('s1');
    const restarted = await ServerApp.create({ port: PORT, host: '127.0.0.1', home: server.home, webDir: null, version: 'test' });
    const res = await restarted.inject({ method: 'GET', url: `/api/projects/${id}/sessions`, headers: { host: `127.0.0.1:${PORT}` } });
    expect(res.json()[0]).toMatchObject({ id: 's1', subagents: [{ id: 'a1' }] });
    await restarted.close();
  });

  it('startet nach beschädigter Sessiondatei leer, statt zu scheitern', async () => {
    await server.app.close();
    const { writeFileSync } = await import('node:fs');
    writeFileSync(server.home.sessionsFile, '{kaputt');
    const restarted = await ServerApp.create({ port: PORT, host: '127.0.0.1', home: server.home, webDir: null, version: 'test' });
    const res = await restarted.inject({ method: 'GET', url: `/api/projects/${id}/sessions`, headers: { host: `127.0.0.1:${PORT}` } });
    expect(res.json()).toEqual([]);
    await restarted.close();
  });

  it('antwortet für ein unbekanntes Projekt mit 404', async () => {
    expect((await server.get('/api/projects/00000000/sessions')).status).toBe(404);
  });
});

describe('Sessions im Ereignisstrom', () => {
  let live: LiveServer;
  beforeEach(async () => (live = await LiveServer.start(5000)));
  afterEach(() => live.server.dispose());

  it('meldet jede Änderung an Sessions als "sessions"', async () => {
    const { id, repo, project } = await live.server.addProject();
    const token = await new TokenStore(project.layout).read();
    const stream = await live.listen(id);
    await LiveServer.until(() => stream.events.length > 0);
    const res = await fetch(`${live.url}/api/hook`, {
      method: 'POST',
      headers: { 'content-type': 'application/json', 'x-harness': '1', 'x-harness-token': token },
      body: JSON.stringify({ project: repo.path, event: { kind: 'session-start', sessionId: 's1', cwd: 'D:/wt' } }),
    });
    expect(res.status).toBe(204);
    expect(await LiveServer.until(() => stream.events.some((e) => e.type === 'sessions'))).toBe(true);
    await stream.stop();
  });
});
