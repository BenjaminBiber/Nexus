import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { DISPATCH_MARKERS } from '@harness/core';
import type { MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';
import { TempRepo } from '../../../core/tests/unit/helpers/TempRepo.js';

const ID = ['-c', 'user.name=Tester', '-c', 'user.email=t@t'];

describe('GET /api/db/tickets/:id/timeline', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;
  let boardId: number;
  const repos: TempRepo[] = [];

  beforeEach(async () => {
    ({ server, backend } = await TestServer.startWithBoard());
    await backend.services.projects.create('ARC', 'Arches', backend.actor());
    boardId = (await backend.services.boards.create('ARC', 'Sprint', backend.actor())).id;
    await backend.services.tickets.create(boardId, { title: 'Login' }, backend.actor());
  });
  afterEach(async () => {
    await server.dispose();
    repos.splice(0).forEach((r) => r.dispose());
  });

  it('liefert die DB-Ereignisse und meldet fehlende lokale Quellen', async () => {
    await backend.services.comments.add('ARC-0001', 'Hallo', backend.actor());
    const res = await server.get('/api/db/tickets/arc-0001/timeline');
    expect(res.status).toBe(200);
    expect(res.json.ticketId).toBe('ARC-0001');
    expect(res.json.events.map((e: { type: string }) => e.type)).toEqual(['field:status', 'comment']);
    expect(res.json.events[0]).toMatchObject({ kind: 'status', actor: { name: 'Florian', type: 'person' } });
    expect(res.json.sources).toMatchObject({ runs: 'unavailable', git: 'unavailable' });
  });

  it('erkennt Agenten nach der Dispatch-Konfiguration', async () => {
    writeFileSync(server.home.dispatchConfigFile, 'dispatcherName: robo\nagentPrefix: azubi-\n');
    await backend.services.comments.add('ARC-0001', `${DISPATCH_MARKERS.START}: an azubi-ARC-0001 übergeben – Branch b, Profil klein (Modell x).`, backend.actor('robo'));
    await backend.services.tickets.claim('ARC-0001', 'azubi-ARC-0001', backend.actor('azubi-ARC-0001'));
    const events = (await server.get('/api/db/tickets/ARC-0001/timeline')).json.events;
    expect(events.find((e: { type: string }) => e.type === 'dispatch:start')).toMatchObject({ kind: 'run', actor: { type: 'agent' }, ref: { profile: 'klein' } });
    expect(events.find((e: { type: string }) => e.type === 'field:assignee')).toMatchObject({ actor: { name: 'azubi-ARC-0001', type: 'agent' } });
  });

  it('ergänzt Lauf-Protokolle mit Ausgang', async () => {
    mkdirSync(server.home.dispatchLogsDir, { recursive: true });
    writeFileSync(join(server.home.dispatchLogsDir, 'ARC-0001-20991016-091500.log'), 'Anweisung\n[dispatch] 2099-10-16T07:30:00.000Z Ende: Exit-Code 1\n');
    writeFileSync(join(server.home.dispatchLogsDir, 'ARC-00010-20991016-091500.log'), 'fremd');
    const res = (await server.get('/api/db/tickets/ARC-0001/timeline')).json;
    expect(res.sources.runs).toBe('ok');
    const runs = res.events.filter((e: { kind: string }) => e.kind === 'run');
    expect(runs.map((e: { type: string }) => e.type)).toEqual(['run-log', 'run-end']);
    expect(runs[0]).toMatchObject({ actor: { name: 'junior-ARC-0001', type: 'agent' }, ref: { runLog: 'ARC-0001-20991016-091500.log', exitCode: 1 } });
    expect(runs[1]).toMatchObject({ at: '2099-10-16T07:30:00.000Z', text: expect.stringContaining('Exit-Code 1') });
  });

  it('übergeht Lauf-Protokolle von vor der Anlage des Tickets (früheres Ticket mit derselben ID)', async () => {
    mkdirSync(server.home.dispatchLogsDir, { recursive: true });
    writeFileSync(join(server.home.dispatchLogsDir, 'ARC-0001-20201021-150715.log'), 'alt\n[dispatch] 2020-10-21T13:27:34.000Z Ende: Exit-Code 0\n');
    writeFileSync(join(server.home.dispatchLogsDir, 'ARC-0001-20991016-091500-plan.log'), 'neu');
    const res = (await server.get('/api/db/tickets/ARC-0001/timeline')).json;
    expect(res.sources.runs).toBe('ok');
    const runs = res.events.filter((e: { kind: string }) => e.kind === 'run');
    expect(runs.map((e: { ref: { runLog: string } }) => e.ref.runLog)).toEqual(['ARC-0001-20991016-091500-plan.log']);
  });

  it('ergänzt die Commits des Ticket-Branches aus dem Repo des Boards', async () => {
    const repo = new TempRepo();
    repos.push(repo);
    repo.git('checkout', '-q', '-b', 'ticket/ARC-0001-login');
    writeFileSync(join(repo.path, 'a.txt'), 'a');
    repo.git('add', '.');
    repo.git(...ID, 'commit', '-q', '-m', 'Login-Formular');
    repo.git('checkout', '-q', '-');
    await backend.repoStore.write(new Map([[boardId, repo.path]]));
    await backend.services.tickets.update('ARC-0001', { branch: 'ticket/ARC-0001-login' }, backend.actor());
    const res = (await server.get('/api/db/tickets/ARC-0001/timeline')).json;
    expect(res.sources.git).toBe('ok');
    expect(res.events.find((e: { type: string }) => e.type === 'commit')).toMatchObject({
      kind: 'git', text: 'Login-Formular', actor: { name: 'Tester', type: 'person' }, ref: { commit: expect.stringMatching(/^[0-9a-f]{40}$/) },
    });
  });

  it('ergänzt bei Zielbranch nur die Commits seit dem Zielbranch', async () => {
    const repo = new TempRepo();
    repos.push(repo);
    const main = repo.git('rev-parse', '--abbrev-ref', 'HEAD');
    repo.git('checkout', '-q', '-b', 'release/1');
    writeFileSync(join(repo.path, 'r.txt'), 'r');
    repo.git('add', '.');
    repo.git(...ID, 'commit', '-q', '-m', 'Stand von release/1');
    repo.git('checkout', '-q', '-b', 'ticket/ARC-0001-login');
    writeFileSync(join(repo.path, 'a.txt'), 'a');
    repo.git('add', '.');
    repo.git(...ID, 'commit', '-q', '-m', 'Login-Formular');
    repo.git('checkout', '-q', main);
    await backend.repoStore.write(new Map([[boardId, repo.path]]));
    await backend.services.tickets.update('ARC-0001', { targetBranch: 'release/1', branch: 'ticket/ARC-0001-login' }, backend.actor());
    const res = (await server.get('/api/db/tickets/ARC-0001/timeline')).json;
    expect(res.sources.git).toBe('ok');
    expect(res.events.filter((e: { type: string }) => e.type === 'commit').map((e: { text: string }) => e.text)).toEqual(['Login-Formular']);
  });

  it('meldet ein unbekanntes Ticket mit 404', async () => {
    expect((await server.get('/api/db/tickets/ARC-0099/timeline')).status).toBe(404);
  });
});
