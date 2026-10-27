import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { BoardRepoPaths } from '@harness/core';
import { afterEach, describe, expect, it } from 'vitest';
import type { BoardRuntimeHolder } from '../../src/dbboard/BoardRuntimeHolder.js';
import { DocTicketService } from '../../src/docs/DocTicketService.js';
import { formatSource, parseSource } from '../../src/docs/DocTicketSource.js';
import type { DocsService } from '../../src/docs/DocsService.js';
import { MEMORY_USER, type MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';

const SPEC = 'docs/superpowers/specs/2026-10-22-docs-rework-design.md';

// Ein vollständiger Entwurf, wie ihn `docs/ticket-drafts` liefert
const draft = (title: string, extra: Record<string, unknown> = {}) => ({
  title, description: `Beschreibung ${title}`, acceptance: `Abnahme ${title}`,
  steps: ['Test schreiben', 'Umsetzen'], dependsOn: [], labels: ['server'],
  source: { heading: 'Ziel', startLine: 3, endLine: 5 },
  ...extra,
});

describe('Tickets aus geprüften Entwürfen anlegen', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;
  let base = '';
  let boardId = 0;
  let pid = '';
  let repoPath = '';
  afterEach(() => server.dispose());

  // Startet Server, Projekt und Board; `withRepoPath` false lässt das Board ohne Repo-Pfad
  async function start(withRepoPath = true): Promise<void> {
    ({ server, backend } = await TestServer.startWithBoard());
    const { repo, id } = await server.addProject();
    pid = id;
    repoPath = repo.path;
    base = `/api/projects/${id}/docs/tickets`;
    await server.post('/api/db/projects', { key: 'ARC', name: 'Arches' });
    boardId = (await server.post('/api/db/boards', { project: 'ARC', name: 'Sprint' })).json.id;
    if (withRepoPath) await new BoardRepoPaths(backend.repoStore, MEMORY_USER.id).set(boardId, repo.path);
    mkdirSync(join(repo.path, 'docs/superpowers/specs'), { recursive: true });
    writeFileSync(join(repo.path, SPEC), '# Docs-Rework\n\n## Ziel\n\nEine Quelle für Docs.\n\n## Plan\n\nSchritt eins.\n');
  }

  const create = (drafts: unknown[], path = SPEC) => server.post(base, { path, drafts });
  const ticket = (id: string) => server.get(`/api/db/tickets/${id}`);

  it('legt die Entwürfe in Eingabereihenfolge an und übersetzt dependsOn in IDs', async () => {
    await start();
    const res = await create([draft('A'), draft('B', { dependsOn: [0] }), draft('C', { dependsOn: [0, 1] })]);
    expect(res.status).toBe(200);
    expect(res.json.skipped).toEqual([]);
    expect(res.json.created.map((c: { index: number }) => c.index)).toEqual([0, 1, 2]);
    const ids = res.json.created.map((c: { id: string }) => c.id);
    expect(ids).toEqual(['ARC-0001', 'ARC-0002', 'ARC-0003']);
    expect((await ticket(ids[1])).json.dependsOn).toEqual([ids[0]]);
    expect((await ticket(ids[2])).json.dependsOn).toEqual([ids[0], ids[1]]);
  });

  it('setzt Status, Planungsbedarf, Labels, Schritte, Abnahme und Spec', async () => {
    await start();
    const id = (await create([draft('A')])).json.created[0].id;
    const detail = (await ticket(id)).json;
    expect(detail.status).toBe('backlog');
    expect(detail.needsPlanning).toBe(true);
    expect(detail.labels).toEqual(['server']);
    expect(detail.acceptance).toBe('Abnahme A');
    expect(detail.spec).toBe(SPEC);
    expect(detail.steps.map((s: { text: string }) => s.text)).toEqual(['Test schreiben', 'Umsetzen']);
  });

  it('hängt die Quelle an die Beschreibung', async () => {
    await start();
    const id = (await create([draft('A')])).json.created[0].id;
    const description = (await ticket(id)).json.description;
    expect(description).toBe(`Beschreibung A\n\nQuelle: ${SPEC} › Ziel (Z. 3–5)`);
  });

  it('überspringt eine Dublette und löst Abhängigkeiten auf das vorhandene Ticket auf', async () => {
    await start();
    const existing = (await server.post(`/api/db/boards/${boardId}/tickets`, { title: 'B' })).json.id;
    const res = await create([draft('A'), draft('B'), draft('C', { dependsOn: [1] })]);
    expect(res.status).toBe(200);
    expect(res.json.created.map((c: { index: number }) => c.index)).toEqual([0, 2]);
    expect(res.json.skipped).toEqual([{ index: 1, reason: 'ticket-duplicate', existingId: existing }]);
    const c = res.json.created[1].id;
    expect((await ticket(c)).json.dependsOn).toEqual([existing]);
  });

  it('überspringt einen kaputten Entwurf, ohne den Rest zu blockieren', async () => {
    await start();
    const res = await create([draft('A'), draft('', { title: '' }), draft('C')]);
    expect(res.json.created.map((c: { index: number }) => c.index)).toEqual([0, 2]);
    expect(res.json.skipped).toHaveLength(1);
    expect(res.json.skipped[0].index).toBe(1);
    expect(res.json.skipped[0].existingId).toBeUndefined();
  });

  it('prüft jeden Entwurf auf dem Server und überspringt unvollständige', async () => {
    await start();
    const noSource = { ...draft('B') } as Record<string, unknown>;
    delete noSource.source;
    const res = await create([draft('A'), draft('X', { acceptance: '' }), noSource, 'kein Objekt', draft('E', { source: { heading: 'Ziel', startLine: 50, endLine: 60 } })]);
    expect(res.status).toBe(200);
    expect(res.json.skipped).toEqual([
      { index: 1, reason: 'draft-invalid' },
      { index: 3, reason: 'draft-invalid' },
    ]);
    expect(res.json.created.map((c: { index: number }) => c.index)).toEqual([0, 2, 4]);
    // ohne oder mit unbrauchbarem Zeilenbereich zeigt die Quelle auf die erste Zeile
    expect((await ticket(res.json.created[1].id)).json.description).toMatch(/\(Z\. 1–1\)$/);
    expect((await ticket(res.json.created[2].id)).json.description).toMatch(/\(Z\. 1–1\)$/);
  });

  it('verwirft Abhängigkeiten auf unbekannte Entwürfe', async () => {
    await start();
    const res = await create([draft('A', { dependsOn: [2] }), draft('B', { dependsOn: [0] })]);
    expect(res.json.created).toHaveLength(2);
    const [a, b] = res.json.created.map((c: { id: string }) => c.id);
    expect((await ticket(a)).json.dependsOn).toEqual([]);
    expect((await ticket(b)).json.dependsOn).toEqual([a]);
  });

  it('liefert den Dokumentpfad als Gruppe in spec-progress', async () => {
    await start();
    await create([draft('A'), draft('B')]);
    const progress = await server.get(`/api/db/boards/${boardId}/spec-progress`);
    expect(progress.status).toBe(200);
    const group = progress.json.find((p: { spec: string }) => p.spec === SPEC);
    expect(group).toBeTruthy();
    expect(group.total).toBe(2);
  });

  it('prüft Pfad und Entwürfe', async () => {
    await start();
    expect((await create([draft('A')], '../geheim.md')).json).toMatchObject({ code: 'doc-not-found' });
    expect((await create([])).json).toMatchObject({ code: 'invalid-input' });
  });

  it('meldet ein Board ohne Repo-Pfad mit einem Hinweis', async () => {
    await start(false);
    const res = await create([draft('A')]);
    expect(res.status).toBeGreaterThanOrEqual(400);
    expect(res.json.message).toMatch(/Repo|Board/);
  });

  describe('Tickets eines Dokuments lesen', () => {
    const list = (path = SPEC) => server.get(`/api/projects/${pid}/docs/tickets?path=${encodeURIComponent(path)}`);

    it('nennt Überschrift und Zeilen aus der Quellenzeile, sortiert nach Anfangszeile', async () => {
      await start();
      await create([
        draft('Spät', { source: { heading: 'Plan', startLine: 7, endLine: 9 } }),
        draft('Früh', { source: { heading: 'Ziel', startLine: 3, endLine: 5 } }),
      ]);
      const res = await list();
      expect(res.status).toBe(200);
      expect(res.json.map((t: { title: string }) => t.title)).toEqual(['Früh', 'Spät']);
      expect(res.json[0]).toMatchObject({ title: 'Früh', heading: 'Ziel', startLine: 3, endLine: 5, status: 'backlog', column: 'backlog' });
      expect(res.json[1]).toMatchObject({ heading: 'Plan', startLine: 7, endLine: 9 });
    });

    it('behält ein Ticket ohne Quellenzeile mit heading null', async () => {
      await start();
      const id = (await create([draft('A')])).json.created[0].id;
      // Die Quellenzeile von Hand aus der Beschreibung entfernt: das Ticket gehört weiter zum Dokument
      await server.patch(`/api/db/tickets/${id}`, { description: 'Ohne Quelle', rev: (await ticket(id)).json.rev });
      const res = await list();
      expect(res.json).toHaveLength(1);
      expect(res.json[0]).toMatchObject({ id, heading: null, startLine: 0, endLine: 0 });
    });

    it('lässt Tickets anderer Dokumente weg', async () => {
      await start();
      await create([draft('A')]);
      const other = 'docs/superpowers/specs/2026-10-22-anderes-design.md';
      writeFileSync(join(repoPath, other), '# Anderes\n\n## Ziel\n\nText.\n');
      expect((await list(other)).json).toEqual([]);
    });

    it('antwortet ohne Repo-Pfad mit einer leeren Liste statt mit einem Fehler', async () => {
      await start(false);
      const res = await list();
      expect(res.status).toBe(200);
      expect(res.json).toEqual([]);
    });
  });
});

describe('Fehler beim Anlegen', () => {
  const source = { root: 'C:/repo', boardId: 1, boardName: 'Sprint', reason: null };
  const docs = { read: () => ({ path: SPEC, content: 'eins\nzwei\ndrei\n' }) } as unknown as DocsService;
  const service = (error: unknown) => new DocTicketService(docs, { run: async () => { throw error; } } as unknown as BoardRuntimeHolder);
  const request = { path: SPEC, drafts: [draft('A'), draft('B')] };

  it('reicht Verbindungs- und Programmfehler weiter statt sie als übersprungen zu melden', async () => {
    const lost = Object.assign(new Error('socket'), { name: 'ConnectionError' });
    await expect(service(lost).create(source, request)).rejects.toBe(lost);
    const bug = new TypeError('x is undefined');
    await expect(service(bug).create(source, request)).rejects.toBe(bug);
  });
});

describe('Quellenzeile schreiben und lesen', () => {
  it('liest zurück, was formatSource geschrieben hat', () => {
    const line = formatSource(SPEC, 'Ziel › Teil', 3, 12);
    expect(parseSource(`Beschreibung\n\n${line}`)).toEqual({ path: SPEC, heading: 'Ziel › Teil', startLine: 3, endLine: 12 });
  });

  it('nimmt die letzte Quellenzeile und meldet ohne eine null', () => {
    const text = `${formatSource('a.md', 'Alt', 1, 2)}\n${formatSource('b.md', 'Neu', 4, 5)}`;
    expect(parseSource(text)?.heading).toBe('Neu');
    expect(parseSource('Nur Beschreibung')).toBeNull();
  });
});
