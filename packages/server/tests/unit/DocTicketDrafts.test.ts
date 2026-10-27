import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { BoardRepoPaths, type RunResult } from '@harness/core';
import { afterEach, describe, expect, it } from 'vitest';
import { MEMORY_USER, type MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';

const SPEC = 'docs/superpowers/specs/2026-10-22-docs-rework-design.md';
const SELECTION = [{ heading: 'Ziel', startLine: 3, endLine: 5, quote: 'Eine Quelle für Docs.' }];

// Antwort von `claude -p --output-format json` mit diesem Text als Ergebnis
const reply = (result: string, cost = 0.02): RunResult =>
  ({ code: 0, timedOut: false, output: JSON.stringify({ type: 'result', subtype: 'success', is_error: false, result, total_cost_usd: cost }) });

// Ein vollständiger Entwurf
const draft = (title: string, extra: Record<string, unknown> = {}) => ({
  title, description: `Beschreibung ${title}`, acceptance: `Abnahme ${title}`,
  steps: ['Test schreiben'], dependsOn: [], labels: ['server'],
  source: { heading: 'Ziel', startLine: 3, endLine: 5 },
  ...extra,
});

describe('Ticket-Entwürfe aus markierten Stellen', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;
  let base = '';
  afterEach(() => server.dispose());

  // Startet Server, Projekt und Board mit Repo-Pfad und legt die Testspec an
  async function start(run: () => Promise<RunResult>): Promise<void> {
    ({ server, backend } = await TestServer.startWithBoard({ ticketDraftClaude: { run } }));
    const { repo, id } = await server.addProject();
    base = `/api/projects/${id}/docs/ticket-drafts`;
    await server.post('/api/db/projects', { key: 'ARC', name: 'Arches' });
    const boardId = (await server.post('/api/db/boards', { project: 'ARC', name: 'Sprint' })).json.id;
    await new BoardRepoPaths(backend.repoStore, MEMORY_USER.id).set(boardId, repo.path);
    mkdirSync(join(repo.path, 'docs/superpowers/specs'), { recursive: true });
    writeFileSync(join(repo.path, SPEC), '# Docs-Rework\n\n## Ziel\n\nEine Quelle für Docs.\n\n## Plan\n\nSchritt eins.\n');
  }

  const create = (body: unknown = { path: SPEC, selections: SELECTION }) => server.post(base, body);

  it('liefert geprüfte Entwürfe mit Quelle, Abhängigkeiten und Kosten', async () => {
    await start(async () => reply(JSON.stringify([draft('A'), draft('B', { dependsOn: [0] })])));
    const res = await create({ path: SPEC, selections: SELECTION, note: 'bitte klein schneiden' });
    expect(res.status).toBe(200);
    expect(res.json.drafts.map((d: { title: string }) => d.title)).toEqual(['A', 'B']);
    expect(res.json.drafts[1].dependsOn).toEqual([0]);
    expect(res.json.drafts[0].source).toEqual({ heading: 'Ziel', startLine: 3, endLine: 5 });
    expect(res.json.costUsd).toBe(0.02);
    expect(typeof res.json.at).toBe('string');
  });

  it('meldet eine Antwort ohne JSON als analysis-failed', async () => {
    await start(async () => reply('Ich würde drei Tickets vorschlagen.'));
    const res = await create();
    expect(res.status).toBe(422);
    expect(res.json).toMatchObject({ code: 'analysis-failed' });
    expect(res.json.message).toContain('nicht lesbar');
  });

  it('verwirft unvollständige Entwürfe und hält die Abhängigkeiten gerade', async () => {
    await start(async () => reply(JSON.stringify([draft('A'), draft('B', { acceptance: '' }), draft('C', { dependsOn: [0, 1] })])));
    const res = await create();
    expect(res.json.drafts.map((d: { title: string }) => d.title)).toEqual(['A', 'C']);
    expect(res.json.drafts[1].dependsOn).toEqual([0]);
    expect(res.json.dropped.length).toBeGreaterThan(0);
  });

  it('bereinigt Kreise in dependsOn und behält alle Entwürfe', async () => {
    await start(async () => reply(JSON.stringify([
      draft('A', { dependsOn: [2] }), draft('B', { dependsOn: [0] }), draft('C', { dependsOn: [1] }),
    ])));
    const res = await create();
    expect(res.json.drafts).toHaveLength(3);
    const edges = res.json.drafts.flatMap((d: { dependsOn: number[] }, to: number) => d.dependsOn.map((from) => [from, to]));
    expect(edges).toHaveLength(2);
    expect(res.json.dropped.join(' ')).toContain('Kreis');
  });

  it('liefert höchstens 15 Entwürfe', async () => {
    await start(async () => reply(JSON.stringify(Array.from({ length: 20 }, (_, i) => draft(`T${i}`)))));
    expect((await create()).json.drafts).toHaveLength(15);
  });

  it('lässt je Projekt nur eine Erzeugung gleichzeitig laufen', async () => {
    let finish: (r: RunResult) => void = () => {};
    let started: () => void = () => {};
    const running = new Promise<void>((resolve) => { started = resolve; });
    let calls = 0;
    // der erste Aufruf hängt, spätere antworten sofort
    await start(() => (calls++ === 0
      ? new Promise<RunResult>((resolve) => { finish = resolve; started(); })
      : Promise.resolve(reply(JSON.stringify([draft('A')])))));
    const first = create();
    await running;
    const second = await create();
    expect(second.status).toBe(409);
    expect(second.json.details).toEqual({ reason: 'drafts-running' });
    finish(reply(JSON.stringify([draft('A')])));
    expect((await first).status).toBe(200);
    // danach ist dasselbe Projekt wieder frei
    expect((await create()).status).toBe(200);
  });

  it('zerlegt mit whole das ganze Dokument ohne Markierungen und lässt bis zu 40 Entwürfe zu', async () => {
    let prompt = '';
    await start(async (...args: unknown[]) => {
      prompt = (args[0] as { prompt: string }).prompt;
      return reply(JSON.stringify(Array.from({ length: 45 }, (_, i) => draft(`T${i}`))));
    });
    const res = await create({ path: SPEC, whole: true });
    expect(res.status).toBe(200);
    expect(res.json.drafts).toHaveLength(40);
    expect(res.json.dropped).toContain('Nur die ersten 40 von 45 Entwürfen übernommen.');
    expect(prompt).toContain('ganzes Dokument');
    expect(prompt).not.toContain('## Markierte Stellen');
    expect(prompt).toContain('9\tSchritt eins.');
  });

  it('bricht die laufende Erzeugung ab und gibt das Projekt wieder frei', async () => {
    let started: () => void = () => {};
    const running = new Promise<void>((resolve) => { started = resolve; });
    let calls = 0;
    // der erste Aufruf endet erst mit dem Abbruch, spätere antworten sofort
    await start((async (input: { signal?: AbortSignal }) => {
      if (calls++ > 0) return reply(JSON.stringify([draft('A')]));
      started();
      await new Promise<void>((resolve) => input.signal?.addEventListener('abort', () => resolve()));
      return { code: null, timedOut: false, output: '' };
    }) as unknown as () => Promise<RunResult>);
    const first = create();
    await running;
    expect((await server.request('DELETE', base, {})).json).toEqual({ cancelled: true });
    expect((await first).json).toMatchObject({ code: 'drafts-cancelled' });
    expect((await server.request('DELETE', base, {})).json).toEqual({ cancelled: false });
    expect((await create()).status).toBe(200);
  });

  it('merkt sich die Entwürfe je Datei, speichert Änderungen und verwirft sie', async () => {
    await start(async () => reply(JSON.stringify([draft('A'), draft('B', { dependsOn: [0] })])));
    const saved = `${base}/saved?path=${encodeURIComponent(SPEC)}`;
    expect((await server.get(saved)).json).toEqual({ result: null });
    const made = (await create()).json;
    expect((await server.get(saved)).json.result).toEqual(made);

    const edited = [{ ...made.drafts[1], title: 'B neu', dependsOn: [] }];
    const put = await server.request('PUT', `${base}/saved`, { path: SPEC, drafts: edited });
    expect(put.status).toBe(200);
    const after = (await server.get(saved)).json.result;
    expect(after.drafts.map((d: { title: string }) => d.title)).toEqual(['B neu']);
    expect(after.costUsd).toBe(made.costUsd);
    expect(after.at).toBe(made.at);

    expect((await server.request('DELETE', saved, {})).status).toBe(200);
    expect((await server.get(saved)).json).toEqual({ result: null });
  });

  it('markiert in der Docs-Liste die Dateien mit gespeicherten Entwürfen', async () => {
    await start(async () => reply(JSON.stringify([draft('A')])));
    const docs = base.replace('/ticket-drafts', '');
    const flagged = async () => (await server.get(docs)).json.docs.find((d: { path: string }) => d.path === SPEC).hasDrafts;
    expect(await flagged()).toBeUndefined();
    await create();
    expect(await flagged()).toBe(true);
    await server.request('DELETE', `${base}/saved?path=${encodeURIComponent(SPEC)}`, {});
    expect(await flagged()).toBeUndefined();
  });

  it('verwirft die gespeicherten Entwürfe, sobald daraus Tickets angelegt sind', async () => {
    await start(async () => reply(JSON.stringify([draft('A')])));
    const made = (await create()).json;
    const tickets = base.replace('ticket-drafts', 'tickets');
    expect((await server.post(tickets, { path: SPEC, drafts: made.drafts })).status).toBe(200);
    expect((await server.get(`${base}/saved?path=${encodeURIComponent(SPEC)}`)).json).toEqual({ result: null });
  });

  it('prüft Pfad und Markierungen, bevor Claude startet', async () => {
    let started = 0;
    await start(async () => { started += 1; return reply('[]'); });
    expect((await create({ path: '../geheim.md', selections: SELECTION })).json).toMatchObject({ code: 'doc-not-found' });
    expect((await create({ path: 'docs/superpowers/specs/fehlt.md', selections: SELECTION })).json.code).toBe('doc-not-found');
    const empty = await create({ path: SPEC, selections: [] });
    expect(empty.status).toBe(422);
    expect(empty.json).toMatchObject({ code: 'invalid-input' });
    expect(started).toBe(0);
  });
});
