import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { TestServer, PORT } from '../helpers/TestServer.js';

type Route = [method: string, path: string, body: unknown];

describe('Sicherheitsmatrix (SC-005)', () => {
  let server: TestServer;
  let routes: Route[];
  let before: string;
  let projectId: string;

  // Sessions und Favoriten als Text, um Veränderungen zu erkennen
  const snapshot = async (): Promise<string> =>
    JSON.stringify([(await server.get(`/api/projects/${projectId}/sessions`)).json, (await server.get(`/api/projects/${projectId}/favorites`)).json]);

  beforeEach(async () => {
    server = await TestServer.start();
    const { id, project } = await server.addProject();
    projectId = id;
    routes = [
      ['POST', '/api/projects', { path: project.root }],
      ['POST', '/api/hook', { project: project.root, event: { kind: 'session-start', sessionId: 'x', cwd: 'D:/x' } }],
      ['POST', `/api/projects/${id}/sessions/x/claim`, { ticket: 'ARC-0001' }],
      ['PUT', `/api/projects/${id}/favorites/x`, { note: 'Eingeschleust' }],
      ['DELETE', `/api/projects/${id}/favorites/x`, {}],
      // Routen der Board-Datenbank: der Schutz greift vor jeder Route (auch ohne board.yaml)
      ['POST', '/api/db/projects', { key: 'EVIL', name: 'Eingeschleust' }],
      ['DELETE', '/api/db/boards/1', {}],
      ['POST', '/api/db/pick-folder', {}],
      ['PATCH', '/api/db/tickets/ARC-0001', { rev: 1, title: 'Überschrieben' }],
      ['POST', '/api/db/tickets/ARC-0001/steps/1', { done: true }],
      ['POST', '/api/db/boards/1/notes', { body: 'Eingeschleust' }],
      ['POST', '/api/db/boards/1/import', { ticketIds: ['ARC-0001'] }],
      // Fragen der Board-Datenbank: Schreibrouten nur mit Schutzkopf
      ['POST', '/api/db/projects/ARC/questions', { title: 'Eingeschleust' }],
      ['PATCH', '/api/db/questions/ARC-F0001', { rev: 1, title: 'Überschrieben' }],
      ['PUT', '/api/db/questions/ARC-F0001/tickets', { rev: 1, ticketIds: ['ARC-0001'] }],
      ['PUT', '/api/db/questions/ARC-F0001/depends', { rev: 1, dependsOn: ['ARC-0001'] }],
      ['POST', '/api/db/questions/ARC-F0001/comments', { text: 'Eingeschleust' }],
      ['POST', '/api/db/questions/ARC-F0001/decision', { rev: 1, text: 'Eingeschleust' }],
      ['POST', '/api/db/questions/ARC-F0001/discard', { rev: 1, reason: 'Eingeschleust' }],
      ['POST', '/api/db/questions/ARC-F0001/reopen', { rev: 1 }],
      // Dispatcher: Schreibrouten nur mit Schutzkopf
      ['PUT', '/api/dispatch/config', { enabled: true }],
      ['POST', '/api/dispatch/run', {}],
      ['POST', '/api/dispatch/pause', { minutes: 5 }],
      ['POST', '/api/dispatch/resume', {}],
      ['POST', '/api/dispatch/enable', {}],
      ['POST', '/api/dispatch/disable', {}],
      ['POST', '/api/dispatch/merge/ARC-0001', {}],
      ['POST', '/api/dispatch/accept/ARC-0001', {}],
      ['POST', '/api/dispatch/research/ARC-F0001', {}],
      ['POST', '/api/dispatch/plan/ARC-0001', {}],
    ];
    before = await snapshot();
  });
  afterEach(() => server.dispose());

  // Sendet eine Anfrage mit eigenen Köpfen (ohne die Standardköpfe der Testhilfe)
  const send = (route: Route, headers: Record<string, string>) =>
    server.app.inject({ method: route[0] as 'POST', url: route[1], headers, payload: JSON.stringify(route[2]) });

  const good = { host: `127.0.0.1:${PORT}`, 'content-type': 'application/json', 'x-harness': '1' };

  it('lehnt jede Schreibroute ohne Pflichtkopf ab', async () => {
    for (const route of routes) {
      const res = await send(route, { host: good.host, 'content-type': 'application/json' });
      expect(res.statusCode, route.join(' ')).toBe(403);
      expect(res.json().code).toBe('header-missing');
    }
  });

  it('lehnt jede Schreibroute mit falschem Inhaltstyp ab (einfache Formularanfragen fremder Seiten)', async () => {
    for (const route of routes) {
      const res = await send(route, { host: good.host, 'content-type': 'text/plain', 'x-harness': '1' });
      expect(res.statusCode, route.join(' ')).toBe(403);
    }
  });

  it('lehnt jede Schreibroute mit fremder Herkunft ab', async () => {
    for (const route of routes) {
      const res = await send(route, { ...good, origin: 'http://evil.example' });
      expect(res.statusCode, route.join(' ')).toBe(403);
      expect(res.json().code).toBe('origin-denied');
    }
  });

  it('lehnt jede Schreibroute mit fremdem Hostnamen ab (DNS-Rebinding)', async () => {
    for (const route of routes) {
      const res = await send(route, { ...good, host: `evil.example:${PORT}` });
      expect(res.statusCode, route.join(' ')).toBe(403);
      expect(res.json().code).toBe('host-denied');
    }
  });

  it('lehnt auch Lesezugriffe mit fremder Herkunft oder fremdem Host ab', async () => {
    for (const path of ['/api/projects', `/api/projects/${projectId}/sessions`, `/api/projects/${projectId}/spec-progress`, '/api/db/active', '/api/health']) {
      expect((await server.get(path, { origin: 'http://evil.example' })).status, path).toBe(403);
      expect((await server.get(path, { host: 'evil.example' })).status, path).toBe(403);
    }
  });

  it('verändert bei abgelehnten Anfragen nichts', async () => {
    for (const route of routes) {
      await send(route, { host: good.host });
      await send(route, { ...good, origin: 'http://evil.example' });
    }
    expect(await snapshot()).toBe(before);
  });

  it('lehnt das Hook-Ereignis trotz richtiger Köpfe mit falschem Token ab und legt keine Session an', async () => {
    const hookRoute = routes.find((r) => r[1] === '/api/hook')!;
    const res = await send(hookRoute, { ...good, 'x-harness-token': 'falsch' });
    expect(res.statusCode).toBe(403);
    expect(res.json().code).toBe('token-invalid');
    expect((await server.get(`/api/projects/${projectId}/sessions`)).json).toEqual([]);
  });

  it('akzeptiert dieselben Anfragen mit richtigen Köpfen (Gegenprobe)', async () => {
    const res = await send(routes[0], { ...good, origin: `http://127.0.0.1:${PORT}` });
    expect(res.statusCode).toBe(200);
  });
});

describe('Schreiben nur über die Kernbibliothek (Constitution IV)', () => {
  // Alle .ts-Dateien eines Ordners
  const collect = (dir: string): string[] =>
    readdirSync(dir).flatMap((name) => {
      const path = join(dir, name);
      return statSync(path).isDirectory() ? collect(path) : name.endsWith('.ts') ? [path] : [];
    });

  it('der Server schreibt keine Dateien selbst, außer dem Protokoll in main.ts', () => {
    const files = collect(resolve(__dirname, '../../src')).filter((f) => !f.endsWith('main.ts'));
    expect(files.length).toBeGreaterThan(10);
    const writers = /\b(writeFile|writeFileSync|appendFile|appendFileSync|mkdir|mkdirSync|rm|rmSync|rename|renameSync|unlink|unlinkSync|copyFile|truncate)\b/;
    for (const file of files) {
      const imports = readFileSync(file, 'utf8')
        .split('\n')
        .filter((line) => /^import .* from 'node:(fs|fs\/promises)'/.test(line))
        .join('\n');
      expect(imports, file).not.toMatch(writers);
    }
  });
});
