import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { BoardRepoPaths, DocHash } from '@harness/core';
import { MEMORY_USER, type MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';

const SPEC = 'docs/superpowers/specs/2026-10-22-docs-rework-design.md';
const PLAN = 'docs/superpowers/plans/2026-10-22-docs-rework.md';

describe('Docs (lesend)', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;
  let base: string;
  let root: string;

  // Legt Projekt und Board an; `withRepoPath` ordnet das Board dem Projektordner zu
  async function setup(withRepoPath = true): Promise<void> {
    const { repo, id } = await server.addProject();
    root = repo.path;
    base = `/api/projects/${id}/docs`;
    await server.post('/api/db/projects', { key: 'ARC', name: 'Arches' });
    const boardId = (await server.post('/api/db/boards', { project: 'ARC', name: 'Sprint' })).json.id;
    if (withRepoPath) await new BoardRepoPaths(backend.repoStore, MEMORY_USER.id).set(boardId, root);
    write('docs/superpowers/specs', '2026-10-22-docs-rework-design.md', '# Docs-Rework\n\nDas Ticket wird angelegt.\n\n## Ziel\n\nEine Quelle.\n');
    write('docs/superpowers/plans', '2026-10-22-docs-rework.md', '# Plan Docs-Rework\n\nSchritt eins.\n');
    write('docs/adr', '0001-entscheidung.md', '# Entscheidung\n\nTickets liegen im Git-Verzeichnis.\n');
    write('.', 'GLOSSARY.md', '# Glossar\n\n**Ticket**: Arbeitseinheit\n');
    write('specs/001-login', 'spec.md', 'alte Spec-Kit-Datei');
    write('.', 'geheim.md', 'geheim');
  }

  // Schreibt eine Datei im Repo des Boards
  function write(dir: string, name: string, text: string): void {
    mkdirSync(join(root, dir), { recursive: true });
    writeFileSync(join(root, dir, name), text);
  }

  beforeEach(async () => {
    ({ server, backend } = await TestServer.startWithBoard());
  });
  afterEach(() => server.dispose());

  it('listet Superpowers-Specs und -Pläne, Glossar und ADRs aus dem Repo des Boards', async () => {
    await setup();
    const res = await server.get(base);
    expect(res.status).toBe(200);
    expect(res.json.source).toMatchObject({ root: root.replace(/\\/g, '/'), boardName: 'Sprint', reason: null });
    const byPath = Object.fromEntries(res.json.docs.map((d: any) => [d.path, d]));
    expect(byPath[SPEC]).toMatchObject({ group: 'spec', kind: 'spec', topic: 'docs-rework', date: '2026-10-22', title: 'Docs-Rework', exists: true });
    expect(byPath[PLAN]).toMatchObject({ group: 'plan', kind: 'plan', topic: 'docs-rework', date: '2026-10-22' });
    expect(byPath[SPEC].mtimeMs).toBeGreaterThan(0);
    expect(byPath['GLOSSARY.md']).toMatchObject({ group: 'glossary', kind: null, exists: true });
    expect(byPath['docs/adr/0001-entscheidung.md']).toMatchObject({ group: 'adr', kind: null });
    expect(byPath['specs/001-login/spec.md']).toBeUndefined();
    expect(byPath['geheim.md']).toBeUndefined();
  });

  it('nennt ohne Board den Grund und liefert keine Dokumente', async () => {
    const { id } = await server.addProject();
    const res = await server.get(`/api/projects/${id}/docs`);
    expect(res.status).toBe(200);
    expect(res.json).toEqual({ source: { root: null, boardId: null, boardName: null, reason: 'no-board' }, docs: [] });
  });

  it('nennt den Grund, wenn dem Board der Repo-Pfad fehlt', async () => {
    await setup(false);
    const boardId = (await server.get('/api/db/boards')).json[0].id;
    await server.post(`/api/db/boards/${boardId}/activate`);
    const res = await server.get(base);
    expect(res.status).toBe(200);
    expect(res.json.source).toMatchObject({ root: null, boardId, reason: 'no-repo-path' });
    expect(res.json.docs).toEqual([]);
  });

  it('liefert Inhalt, Hash und Abschnitte freigegebener Dateien', async () => {
    await setup();
    const res = await server.get(`${base}/file?path=${encodeURIComponent(SPEC)}`);
    expect(res.status).toBe(200);
    expect(res.json.path).toBe(SPEC);
    expect(res.json.content).toContain('Das Ticket wird angelegt.');
    expect(res.json.hash).toMatch(/^[0-9a-f]{64}$/);
    expect(res.json.sections).toEqual([
      { heading: 'Docs-Rework', level: 1, startLine: 1, endLine: 8 },
      { heading: 'Ziel', level: 2, startLine: 5, endLine: 8 },
    ]);
    expect((await server.get(`${base}/file?path=GLOSSARY.md`)).status).toBe(200);
    expect((await server.get(`${base}/file?path=${encodeURIComponent(PLAN)}`)).status).toBe(200);
  });

  it('weist nicht freigegebene Pfade und Ausbruchsversuche mit 404 ab', async () => {
    await setup();
    const forbidden = [
      'README.md', 'geheim.md', 'specs/001-login/spec.md', '../geheim.md', '../../etc/passwd', '/etc/passwd',
      'C:/Windows/win.ini', 'docs/superpowers/specs/../../../geheim.md', 'docs\\superpowers\\specs\\..\\..\\..\\geheim.md',
      '%2e%2e/geheim.md', '.git/config', 'docs/adr/../../geheim.md', 'docs/adr/', 'docs/superpowers/specs/',
    ];
    for (const path of forbidden) {
      const res = await server.get(`${base}/file?path=${encodeURIComponent(path)}`);
      expect(res.status, path).toBe(404);
      expect(res.text, path).not.toContain('geheim');
    }
  });

  it('meldet eine freigegebene, aber fehlende Datei mit 404', async () => {
    await setup();
    const res = await server.get(`${base}/file?path=${encodeURIComponent('docs/superpowers/plans/2026-01-01-fehlt.md')}`);
    expect(res.status).toBe(404);
    expect(res.json.message).toContain('2026-01-01-fehlt.md');
  });

  it('verlangt den Parameter path', async () => {
    await setup();
    expect((await server.get(`${base}/file`)).status).toBe(404);
  });

  it('meldet ohne Quelle auch beim Lesen 404', async () => {
    const { id } = await server.addProject();
    expect((await server.get(`/api/projects/${id}/docs/file?path=GLOSSARY.md`)).status).toBe(404);
  });

  it('sucht ohne Beachtung der Schreibweise mit Datei, Zeile und Auszug', async () => {
    await setup();
    const res = await server.get(`${base}/search?q=ticket`);
    expect(res.status).toBe(200);
    const hits = res.json as Array<{ path: string; line: number; excerpt: string }>;
    expect(hits.map((h) => h.path).sort()).toEqual(['GLOSSARY.md', SPEC, 'docs/adr/0001-entscheidung.md'].sort());
    const spec = hits.find((h) => h.path === SPEC)!;
    expect(spec.line).toBe(3);
    expect(spec.excerpt).toContain('Das Ticket wird angelegt.');
  });

  it('durchsucht keine nicht freigegebenen Dateien', async () => {
    await setup();
    expect((await server.get(`${base}/search?q=geheim`)).json).toEqual([]);
    expect((await server.get(`${base}/search?q=Spec-Kit`)).json).toEqual([]);
  });

  it('verlangt mindestens zwei Zeichen', async () => {
    await setup();
    const res = await server.get(`${base}/search?q=a`);
    expect(res.status).toBe(422);
    expect(res.json.code).toBe('query-short');
  });
});

describe('Docs (schreibend)', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;
  let base: string;
  let root: string;

  // Legt Projekt, Board und die freigegebenen Dateien an; gibt die URL der Schreibroute zurück
  async function setup(): Promise<void> {
    const { repo, id } = await server.addProject();
    root = repo.path;
    base = `/api/projects/${id}/docs`;
    await server.post('/api/db/projects', { key: 'ARC', name: 'Arches' });
    const boardId = (await server.post('/api/db/boards', { project: 'ARC', name: 'Sprint' })).json.id;
    await new BoardRepoPaths(backend.repoStore, MEMORY_USER.id).set(boardId, root);
    write('docs/superpowers/specs', '2026-10-22-docs-rework-design.md', '# Docs-Rework\n\nEinleitung.\n\n## Ziel\n\nEine Quelle.\n');
    write('docs/superpowers/plans', '2026-10-22-docs-rework.md', '# Plan Docs-Rework\n\nSchritt eins.\n');
    write('docs/adr', '0001-entscheidung.md', '# Entscheidung\n\nTickets liegen im Git-Verzeichnis.\n');
    write('.', 'GLOSSARY.md', '# Glossar\n\n**Ticket**: Arbeitseinheit\n');
    write('.', 'geheim.md', 'geheim');
  }

  // Schreibt eine Datei im Repo des Boards
  function write(dir: string, name: string, text: string): void {
    mkdirSync(join(root, dir), { recursive: true });
    writeFileSync(join(root, dir, name), text);
  }

  // Inhalt einer Datei im Repo des Boards
  function read(path: string): string {
    return readFileSync(join(root, path), 'utf8');
  }

  // Aktueller Hash eines freigegebenen Dokuments (so, wie der Editor ihn beim Lesen bekommt)
  async function hashOf(path: string): Promise<string> {
    return (await server.get(`${base}/file?path=${encodeURIComponent(path)}`)).json.hash;
  }

  beforeEach(async () => {
    ({ server, backend } = await TestServer.startWithBoard());
  });
  afterEach(() => server.dispose());

  it('ersetzt einen Abschnitt und liefert neuen Hash samt Abschnitten', async () => {
    await setup();
    const res = await server.put(`${base}/section`, {
      path: SPEC, startLine: 5, endLine: 7, content: '## Neues Ziel\n\nZwei Quellen.', expectedHash: await hashOf(SPEC),
    });
    expect(res.status).toBe(200);
    const after = read(SPEC);
    expect(after).toBe('# Docs-Rework\n\nEinleitung.\n\n## Neues Ziel\n\nZwei Quellen.\n');
    expect(res.json.path).toBe(SPEC);
    expect(res.json.hash).toBe(DocHash.of(after));
    expect(res.json.sections).toEqual([
      { heading: 'Docs-Rework', level: 1, startLine: 1, endLine: 8 },
      { heading: 'Neues Ziel', level: 2, startLine: 5, endLine: 8 },
    ]);
  });

  it('lässt von zwei gleichzeitigen Änderungen gegen denselben Stand nur eine durch', async () => {
    await setup();
    const hash = await hashOf(SPEC);
    const [a, b] = await Promise.all([
      server.put(`${base}/section`, { path: SPEC, startLine: 5, endLine: 7, content: '## A', expectedHash: hash }),
      server.put(`${base}/section`, { path: SPEC, startLine: 5, endLine: 7, content: '## B', expectedHash: hash }),
    ]);
    expect([a.status, b.status].sort()).toEqual([200, 409]);
    const winner = a.status === 200 ? '## A' : '## B';
    expect(read(SPEC)).toBe(`# Docs-Rework\n\nEinleitung.\n\n${winner}\n`);
  });

  it('lehnt einen veralteten Hash mit 409 ab und lässt die Datei unverändert', async () => {
    await setup();
    const before = read(SPEC);
    const res = await server.put(`${base}/section`, {
      path: SPEC, startLine: 5, endLine: 7, content: '## Anders', expectedHash: DocHash.of('ganz andere Fassung'),
    });
    expect(res.status).toBe(409);
    expect(res.json.code).toBe('doc-changed');
    expect(res.json.details.hash).toBe(DocHash.of(before));
    expect(read(SPEC)).toBe(before);
  });

  it('lehnt ungültige Zeilenbereiche mit 400 ab', async () => {
    await setup();
    const before = read(SPEC);
    const expectedHash = await hashOf(SPEC);
    for (const range of [[0, 2], [5, 4], [1, 99], [-1, 1], [1.5, 2]]) {
      const res = await server.put(`${base}/section`, { path: SPEC, startLine: range[0], endLine: range[1], content: 'x', expectedHash });
      expect(res.status, JSON.stringify(range)).toBe(400);
      expect(res.json.code, JSON.stringify(range)).toBe('doc-range-invalid');
    }
    expect(read(SPEC)).toBe(before);
  });

  it('lehnt einen unvollständigen Body mit 422 ab', async () => {
    await setup();
    const before = read(SPEC);
    const expectedHash = await hashOf(SPEC);
    for (const body of [{}, { path: SPEC, startLine: 1, endLine: 2, content: 'x' }, { startLine: 1, endLine: 2, content: 'x', expectedHash }, { path: SPEC, startLine: 1, endLine: 2, expectedHash }]) {
      const res = await server.put(`${base}/section`, body);
      expect(res.status, JSON.stringify(body)).toBe(422);
      expect(res.json.code, JSON.stringify(body)).toBe('request-invalid');
    }
    expect(read(SPEC)).toBe(before);
  });

  it('erlaubt einen leeren Inhalt (Abschnitt löschen)', async () => {
    await setup();
    const res = await server.put(`${base}/section`, { path: PLAN, startLine: 3, endLine: 3, content: '', expectedHash: await hashOf(PLAN) });
    expect(res.status).toBe(200);
    expect(read(PLAN)).toBe('# Plan Docs-Rework\n\n\n');
  });

  it('lässt Glossar und ADRs nicht schreiben', async () => {
    await setup();
    for (const path of ['GLOSSARY.md', 'docs/adr/0001-entscheidung.md']) {
      const before = read(path);
      const res = await server.put(`${base}/section`, { path, startLine: 1, endLine: 1, content: '# Neu', expectedHash: await hashOf(path) });
      expect(res.status, path).toBe(422);
      expect(res.json.code, path).toBe('doc-not-writable');
      expect(read(path), path).toBe(before);
    }
  });

  it('weist nicht freigegebene Pfade und Ausbruchsversuche mit 404 ab', async () => {
    await setup();
    for (const path of ['geheim.md', '../geheim.md', 'docs/superpowers/specs/../../../geheim.md', 'docs\\superpowers\\specs\\x.md', '/etc/passwd', 'docs/superpowers/specs/fehlt.md']) {
      const res = await server.put(`${base}/section`, { path, startLine: 1, endLine: 1, content: 'x', expectedHash: 'egal' });
      expect(res.status, path).toBe(404);
      expect(res.json.code, path).toBe('doc-not-found');
    }
    expect(read('geheim.md')).toBe('geheim');
  });

  it('behält CRLF als Zeilenende der Datei bei', async () => {
    await setup();
    write('docs/superpowers/plans', '2026-10-22-docs-rework.md', '# Plan\r\n\r\nAlt.\r\n');
    const res = await server.put(`${base}/section`, { path: PLAN, startLine: 3, endLine: 3, content: 'Neu.', expectedHash: await hashOf(PLAN) });
    expect(res.status).toBe(200);
    const after = read(PLAN);
    expect(after).toBe('# Plan\r\n\r\nNeu.\r\n');
    expect(/(^|[^\r])\n/.test(after)).toBe(false);
  });
});
