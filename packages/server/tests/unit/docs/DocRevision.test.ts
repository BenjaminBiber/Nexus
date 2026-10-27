import { describe, it, expect, afterEach } from 'vitest';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { BoardRepoPaths, DocHash, type RunResult } from '@harness/core';
import { MEMORY_USER, type MemoryBoardBackend } from '../../helpers/MemoryBoardBackend.js';
import { TestServer } from '../../helpers/TestServer.js';

const SPEC = 'docs/superpowers/specs/2026-10-22-docs-rework-design.md';
const CONTENT = '# Docs-Rework\n\nEinleitung.\n\n## Ziel\n\nEine Quelle.\nZweite Zeile.\n\n## Offen\n\nNichts.\n';

// Antwort von `claude -p --output-format json`
const reply = (result: string): RunResult => ({
  code: 0, timedOut: false,
  output: JSON.stringify({ type: 'result', subtype: 'success', is_error: false, result, total_cost_usd: 0.01 }),
});

describe('Docs umschreiben (POST docs/revise)', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;
  let base: string;
  let root: string;
  // Was der Ersatz für Claude als Letztes bekommen hat
  let seen: { prompt: string; args: string[] } | null;

  // Startet den Server mit einem Claude-Ersatz und legt Projekt, Board und Spec an
  async function setup(run: (input: any) => Promise<RunResult>): Promise<void> {
    seen = null;
    ({ server, backend } = await TestServer.startWithBoard({
      reviseClaude: {
        run: async (input: any) => {
          seen = { prompt: input.prompt, args: input.args };
          return run(input);
        },
      },
    }));
    const { repo, id } = await server.addProject();
    root = repo.path;
    base = `/api/projects/${id}/docs`;
    await server.post('/api/db/projects', { key: 'ARC', name: 'Arches' });
    const boardId = (await server.post('/api/db/boards', { project: 'ARC', name: 'Sprint' })).json.id;
    await new BoardRepoPaths(backend.repoStore, MEMORY_USER.id).set(boardId, root);
    mkdirSync(join(root, 'docs/superpowers/specs'), { recursive: true });
    writeFileSync(join(root, SPEC), CONTENT);
  }

  // Body mit gültigem Hash für die Zeilen „Eine Quelle.“ und „Zweite Zeile.“
  const body = (over: Record<string, unknown> = {}) => ({
    path: SPEC, startLine: 7, endLine: 8, instruction: 'Kürzer fassen.', expectedHash: DocHash.of(CONTENT), ...over,
  });

  afterEach(() => server.dispose());

  it('schickt Abschnitt, Bereich und Anweisung an Claude und liefert die Antwort als Vorschlag', async () => {
    await setup(async () => reply('Eine einzige Quelle.'));
    const res = await server.post(`${base}/revise`, body());
    expect(res.status).toBe(200);
    expect(res.json).toEqual({
      original: 'Eine Quelle.\nZweite Zeile.',
      proposal: 'Eine einzige Quelle.',
      startLine: 7, endLine: 8, hash: DocHash.of(CONTENT),
    });
    expect(seen!.prompt).toContain('Eine Quelle.\nZweite Zeile.');
    expect(seen!.prompt).toContain('Kürzer fassen.');
    // umgebender Abschnitt „## Ziel“ als Kontext, aber nicht der Abschnitt „## Offen“
    expect(seen!.prompt).toContain('## Ziel');
    expect(seen!.prompt).not.toContain('## Offen');
    expect(seen!.args).toEqual(expect.arrayContaining(['--max-turns', '1', '--disallowedTools', 'Bash']));
    // Die Route schreibt nichts
    expect(readFileSync(join(root, SPEC), 'utf8')).toBe(CONTENT);
  });

  it('entfernt einen Markdown-Zaun, der die ganze Antwort umschließt', async () => {
    await setup(async () => reply('```markdown\nEine einzige Quelle.\n```\n'));
    const res = await server.post(`${base}/revise`, body());
    expect(res.status).toBe(200);
    expect(res.json.proposal).toBe('Eine einzige Quelle.');
  });

  it('antwortet auf einen veralteten Hash mit 409', async () => {
    await setup(async () => reply('x'));
    const res = await server.post(`${base}/revise`, body({ expectedHash: 'veraltet' }));
    expect(res.status).toBe(409);
    expect(res.json.code).toBe('doc-changed');
    expect(res.json.details.hash).toBe(DocHash.of(CONTENT));
  });

  it('meldet einen fehlenden Claude-Befehl als analysis-failed', async () => {
    await setup(async () => ({ code: 127, timedOut: false, output: '' }));
    const res = await server.post(`${base}/revise`, body());
    expect(res.status).toBe(422);
    expect(res.json.code).toBe('analysis-failed');
    expect(res.json.message).toContain('nicht gefunden');
  });

  it('meldet einen Zeitüberschritt als analysis-failed', async () => {
    await setup(async () => ({ code: null as unknown as number, timedOut: true, output: '' }));
    const res = await server.post(`${base}/revise`, body());
    expect(res.status).toBe(422);
    expect(res.json.message).toContain('5 Minuten');
  });

  it('meldet einen Exit-Code ungleich 0 mit Auszug', async () => {
    await setup(async () => ({ code: 3, timedOut: false, output: 'kaputt im Modell' }));
    const res = await server.post(`${base}/revise`, body());
    expect(res.status).toBe(422);
    expect(res.json.message).toContain('Exit-Code 3');
    expect(res.json.message).toContain('kaputt im Modell');
  });

  it('meldet eine leere Antwort als analysis-failed', async () => {
    await setup(async () => reply('   \n'));
    const res = await server.post(`${base}/revise`, body());
    expect(res.status).toBe(422);
    expect(res.json.code).toBe('analysis-failed');
  });

  it('weist einen Bereich hinter dem Dateiende zurück', async () => {
    await setup(async () => reply('x'));
    const res = await server.post(`${base}/revise`, body({ startLine: 7, endLine: 999 }));
    expect(res.status).toBe(400);
    expect(res.json.code).toBe('doc-range-invalid');
  });

  it('weist startLine 0 schon im Body zurück', async () => {
    await setup(async () => reply('x'));
    const res = await server.post(`${base}/revise`, body({ startLine: 0, endLine: 2 }));
    expect(res.status).toBe(400);
    expect(res.json.code).toBe('doc-range-invalid');
  });

  it('verlangt Pfad, Hash und Anweisung', async () => {
    await setup(async () => reply('x'));
    const res = await server.post(`${base}/revise`, body({ instruction: '  ' }));
    expect(res.status).toBe(422);
    expect(res.json.code).toBe('request-invalid');
  });

  it('antwortet auf eine zweite Anfrage während eines laufenden Laufs mit 409', async () => {
    let release: () => void = () => {};
    const gate = new Promise<void>((resolve) => { release = resolve; });
    await setup(async () => { await gate; return reply('fertig'); });
    const first = server.post(`${base}/revise`, body());
    // warten, bis der erste Lauf wirklich bei Claude angekommen ist
    while (seen === null) await new Promise((r) => setTimeout(r, 5));
    const second = await server.post(`${base}/revise`, body());
    expect(second.status).toBe(409);
    expect(second.json.details.reason).toBe('revise-running');
    release();
    expect((await first).status).toBe(200);
  });

  it('gibt gesperrte und ausgebrochene Pfade als nicht gefunden aus', async () => {
    await setup(async () => reply('x'));
    for (const path of ['GLOSSARY.md', '../geheim.md']) {
      const res = await server.post(`${base}/revise`, body({ path }));
      expect(res.status).toBe(404);
      expect(res.json.code).toBe('doc-not-found');
    }
  });
});
