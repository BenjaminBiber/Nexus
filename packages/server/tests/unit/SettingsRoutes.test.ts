import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { TestServer } from '../helpers/TestServer.js';

describe('SettingsRoutes', () => {
  let server: TestServer;
  let base: string;

  beforeEach(async () => {
    server = await TestServer.start();
    base = `/api/projects/${(await server.addProject()).id}`;
  });
  afterEach(() => server.dispose());

  it('liefert Projekt und Token, aber keine Spalten und keine Sicherung', async () => {
    const res = await server.get(`${base}/settings`);
    expect(res.status).toBe(200);
    expect(res.json.project).toMatchObject({ name: 'Mein Projekt ä' });
    expect(res.json.token).toMatch(/^[0-9a-f]{64}$/);
    expect(Object.keys(res.json).sort()).toEqual(['project', 'token']);
  });

  it('kennt weder Export noch Sicherung', async () => {
    expect((await server.post(`${base}/export`, { target: '/x' })).status).toBe(404);
    expect((await server.post(`${base}/backup`, {})).status).toBe(404);
  });

  it('meldet ein unbekanntes Projekt mit 404', async () => {
    expect((await server.get('/api/projects/gibtsnicht/settings')).status).toBe(404);
  });
});
