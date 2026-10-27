import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { existsSync, mkdirSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { BoardRepoPaths } from '@harness/core';
import { MEMORY_USER, type MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';

const BODY = { text: 'Bitte präzisieren', quote: 'Das Ticket', heading: 'Login', lines: [3, 3] };
const SPEC = 'docs/superpowers/specs/2026-10-22-login-design.md';

describe('Kommentare zu Docs', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;
  let base: string;
  let root: string;
  const url = (path = SPEC, id = '') => `${base}/comments${id ? `/${id}` : ''}?path=${encodeURIComponent(path)}`;

  // Projekt und Board anlegen; `withRepoPath` ordnet das Board dem Projektordner zu
  async function setup(withRepoPath = true): Promise<void> {
    const { repo, id } = await server.addProject();
    root = repo.path;
    base = `/api/projects/${id}/docs`;
    await server.post('/api/db/projects', { key: 'ARC', name: 'Arches' });
    const boardId = (await server.post('/api/db/boards', { project: 'ARC', name: 'Sprint' })).json.id;
    if (withRepoPath) await new BoardRepoPaths(backend.repoStore, MEMORY_USER.id).set(boardId, root);
    mkdirSync(join(root, 'docs', 'superpowers', 'specs'), { recursive: true });
    writeFileSync(join(root, 'docs', 'superpowers', 'specs', '2026-10-22-login-design.md'), '# Login\n\nDas Ticket wird angelegt.\n');
    writeFileSync(join(root, 'geheim.md'), 'geheim');
  }

  beforeEach(async () => {
    ({ server, backend } = await TestServer.startWithBoard());
  });
  afterEach(() => server.dispose());

  it('legt Kommentare an, liest, ändert und löscht sie', async () => {
    await setup();
    expect((await server.get(url())).json).toEqual([]);
    const created = await server.request('POST', url(), BODY);
    expect(created.status).toBe(200);
    expect(created.json).toMatchObject({ ...BODY, status: 'open' });
    const id = created.json.id as string;
    expect(existsSync(join(root, 'docs', 'superpowers', 'specs', '2026-10-22-login-design.comments.json'))).toBe(true);
    const patched = await server.request('PATCH', url(SPEC, id), { status: 'resolved' });
    expect(patched.json.status).toBe('resolved');
    expect((await server.delete(url(SPEC, id))).json).toEqual({ ok: true });
    expect((await server.get(url())).json).toEqual([]);
  });

  it('weist nicht freigegebene Pfade ab und schreibt dort nichts', async () => {
    await setup();
    for (const path of ['geheim.md', '../geheim.md', 'specs/001-login/spec.md']) {
      expect((await server.request('POST', url(path), BODY)).status, path).toBe(404);
    }
    expect(existsSync(join(root, 'geheim.comments.json'))).toBe(false);
  });

  it('weist ohne Board alle vier Zugriffe ab und legt keine Datei an', async () => {
    await setup(false);
    expect((await server.get(url())).status).toBe(404);
    expect((await server.request('POST', url(), BODY)).status).toBe(404);
    expect((await server.request('PATCH', url(SPEC, 'K-00000000'), { text: 'x' })).status).toBe(404);
    expect((await server.delete(url(SPEC, 'K-00000000'))).status).toBe(404);
    expect(readdirSync(join(root, 'docs', 'superpowers', 'specs'))).toEqual(['2026-10-22-login-design.md']);
  });

  it('verlangt Text und Zitat', async () => {
    await setup();
    const res = await server.request('POST', url(), { ...BODY, text: '  ' });
    expect(res.status).toBe(422);
    expect(res.json.code).toBe('comment-invalid');
  });

  it('meldet unbekannte Kommentare mit 404', async () => {
    await setup();
    expect((await server.request('PATCH', url(SPEC, 'K-00000000'), { text: 'x' })).status).toBe(404);
  });
});
