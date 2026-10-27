import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { BoardRepoPaths } from '@harness/core';
import { afterEach, describe, expect, it } from 'vitest';
import type { OpenTarget } from '../../src/docs/FileOpener.js';
import { MEMORY_USER } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';

const SPEC = 'docs/superpowers/specs/2026-10-22-docs-rework-design.md';

describe('Dokument im Explorer oder in VS Code öffnen', () => {
  let server: TestServer;
  afterEach(() => server.dispose());

  // Startet Server und Board mit Repo-Pfad, legt die Spec an und merkt sich, was geöffnet wurde
  async function start() {
    const opened: Array<[string, OpenTarget]> = [];
    const started = await TestServer.startWithBoard({ fileOpener: { open: async (file, target) => void opened.push([file, target]) } });
    server = started.server;
    const { repo, id } = await server.addProject();
    await server.post('/api/db/projects', { key: 'ARC', name: 'Arches' });
    const boardId = (await server.post('/api/db/boards', { project: 'ARC', name: 'Sprint' })).json.id;
    await new BoardRepoPaths(started.backend.repoStore, MEMORY_USER.id).set(boardId, repo.path);
    mkdirSync(join(repo.path, 'docs/superpowers/specs'), { recursive: true });
    writeFileSync(join(repo.path, SPEC), '# Spec\n');
    return { url: `/api/projects/${id}/docs/open`, repo: repo.path, opened };
  }

  it('öffnet die Datei mit vollem Pfad im gewünschten Programm', async () => {
    const { url, repo, opened } = await start();
    expect((await server.post(url, { path: SPEC, target: 'explorer' })).status).toBe(200);
    expect((await server.post(url, { path: SPEC, target: 'vscode' })).status).toBe(200);
    expect(opened).toEqual([[join(repo, SPEC), 'explorer'], [join(repo, SPEC), 'vscode']]);
  });

  it('öffnet nichts außerhalb der freigegebenen Dokumente und kein unbekanntes Ziel', async () => {
    const { url, opened } = await start();
    expect((await server.post(url, { path: '../geheim.md', target: 'explorer' })).json.code).toBe('doc-not-found');
    expect((await server.post(url, { path: SPEC, target: 'notepad' })).json.code).toBe('request-invalid');
    expect(opened).toEqual([]);
  });
});
