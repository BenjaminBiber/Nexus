import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { BoardRepoPaths, type RunResult } from '@harness/core';
import { afterEach, describe, expect, it } from 'vitest';
import { MEMORY_USER, type MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';

const SPEC = 'docs/superpowers/specs/2026-10-22-docs-rework-design.md';

// Antwort von `claude -p --output-format json` mit diesem Text als Ergebnis
const reply = (result: string, cost = 0.01): RunResult =>
  ({ code: 0, timedOut: false, output: JSON.stringify({ type: 'result', subtype: 'success', is_error: false, result, total_cost_usd: cost }) });

// Ein Fragen-Entwurf, wie ihn `docs/question-drafts` liefert
const question = (title: string, extra: Record<string, unknown> = {}) => ({
  title, description: `Kontext zu ${title}`, source: { heading: 'Ziel', startLine: 3, endLine: 5 }, ...extra,
});

describe('Fragen aus einem Dokument', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;
  let pid = '';
  let prompt = '';
  afterEach(() => server.dispose());

  // Startet Server, Projekt und Board mit Repo-Pfad und legt die Testspec an; `run` spielt Claude
  async function start(run: () => Promise<RunResult> = async () => reply('[]')): Promise<void> {
    ({ server, backend } = await TestServer.startWithBoard({
      ticketDraftClaude: { run: async (opts: { prompt: string }) => { prompt = opts.prompt; return run(); } },
    }));
    const { repo, id } = await server.addProject();
    pid = id;
    await server.post('/api/db/projects', { key: 'ARC', name: 'Arches' });
    const boardId = (await server.post('/api/db/boards', { project: 'ARC', name: 'Sprint' })).json.id;
    await new BoardRepoPaths(backend.repoStore, MEMORY_USER.id).set(boardId, repo.path);
    mkdirSync(join(repo.path, 'docs/superpowers/specs'), { recursive: true });
    writeFileSync(join(repo.path, SPEC), '# Docs-Rework\n\n## Ziel\n\nWelcher Speicher? TBD.\n\n## Plan\n\nSchritt eins.\n');
  }

  const drafts = () => server.post(`/api/projects/${pid}/docs/question-drafts`, { path: SPEC });
  const create = (list: unknown[]) => server.post(`/api/projects/${pid}/docs/questions`, { path: SPEC, drafts: list });

  it('liefert geprüfte Entwürfe mit Quelle und Kosten; das Dokument steht mit Zeilennummern im Prompt', async () => {
    await start(async () => reply(JSON.stringify([question('Welcher Speicher?'), question(''), question('Wie viele?', { source: { heading: 'Plan', startLine: 99, endLine: 120 } })])));
    const res = await drafts();
    expect(res.status).toBe(200);
    expect(res.json.drafts.map((d: { title: string }) => d.title)).toEqual(['Welcher Speicher?', 'Wie viele?']);
    expect(res.json.drafts[0].source).toEqual({ heading: 'Ziel', startLine: 3, endLine: 5 });
    // Ein Zeilenbereich außerhalb des Dokuments wird zur ersten Zeile
    expect(res.json.drafts[1].source).toEqual({ heading: 'Plan', startLine: 1, endLine: 1 });
    expect(res.json.dropped).toHaveLength(1);
    expect(res.json.costUsd).toBe(0.01);
    expect(prompt).toContain('5\tWelcher Speicher? TBD.');
  });

  it('liefert ohne offene Fragen eine leere Liste und meldet unlesbare Antworten', async () => {
    await start(async () => reply('[]'));
    expect((await drafts()).json.drafts).toEqual([]);
    await server.dispose();
    await start(async () => reply('Keine Fragen gefunden.'));
    const res = await drafts();
    expect(res.status).toBe(422);
    expect(res.json).toMatchObject({ code: 'analysis-failed' });
  });

  it('legt Fragen mit Quelle und Anlass an und verknüpft die Tickets derselben Überschrift', async () => {
    await start();
    const ticket = await server.post(`/api/projects/${pid}/docs/tickets`, {
      path: SPEC,
      drafts: [{ title: 'Speicher bauen', description: 'B', acceptance: 'A', steps: [], dependsOn: [], labels: [], source: { heading: 'Ziel', startLine: 3, endLine: 5 } }],
    });
    const ticketId = ticket.json.created[0].id;
    const res = await create([question('Welcher Speicher?'), { title: '' }, question('Wie viele Schritte?', { source: { heading: 'Plan', startLine: 7, endLine: 9 } })]);
    expect(res.status).toBe(200);
    expect(res.json.created.map((c: { index: number }) => c.index)).toEqual([0, 2]);
    expect(res.json.skipped).toEqual([{ index: 1, reason: 'draft-invalid' }]);

    const first = (await server.get(`/api/db/questions/${res.json.created[0].id}`)).json;
    expect(first.title).toBe('Welcher Speicher?');
    expect(first.description).toBe(`Kontext zu Welcher Speicher?\n\nQuelle: ${SPEC} › Ziel (Z. 3–5)`);
    expect(first.occasion).toBe(`Dokument ${SPEC}`);
    expect(first.state).toBe('open');
    expect(first.tickets.map((t: { id: string }) => t.id)).toEqual([ticketId]);
    const second = (await server.get(`/api/db/questions/${res.json.created[1].id}`)).json;
    expect(second.tickets).toEqual([]);
  });

  it('entscheidet Fragen mit Antwort gleich, Fragen ohne Antwort bleiben offen', async () => {
    await start();
    const res = await create([question('Welcher Speicher?', { answer: '  SQLite lokal, MSSQL im Team.  ' }), question('Wie viele?', { answer: '   ' })]);
    expect(res.json.created).toEqual([
      { index: 0, id: expect.any(String), answered: true },
      { index: 1, id: expect.any(String), answered: false },
    ]);
    const answered = (await server.get(`/api/db/questions/${res.json.created[0].id}`)).json;
    expect(answered.state).toBe('answered');
    expect(answered.decision).toMatchObject({ outcome: 'answered', text: 'SQLite lokal, MSSQL im Team.', option: null });
    expect((await server.get(`/api/db/questions/${res.json.created[1].id}`)).json.state).toBe('open');
  });

  it('übernimmt keine Antwort aus Claudes Entwürfen', async () => {
    await start(async () => reply(JSON.stringify([question('Welcher Speicher?', { answer: 'erfunden' })])));
    expect((await drafts()).json.drafts[0]).not.toHaveProperty('answer');
  });

  it('verlangt mindestens einen Entwurf', async () => {
    await start();
    const res = await create([]);
    expect(res.status).toBe(422);
    expect(res.json.code).toBe('invalid-input');
  });

  it('bricht eine laufende Erzeugung ab und lässt keine zweite parallel zu', async () => {
    let release: (r: RunResult) => void = () => {};
    await start(() => new Promise<RunResult>((resolve) => { release = resolve; }));
    const pending = drafts();
    await new Promise((r) => setTimeout(r, 50));
    expect((await drafts()).json).toMatchObject({ code: 'conflict' });
    expect((await server.delete(`/api/projects/${pid}/docs/question-drafts`)).json).toEqual({ cancelled: true });
    release(reply('[]'));
    expect((await pending).json).toMatchObject({ code: 'drafts-cancelled' });
  });
});
