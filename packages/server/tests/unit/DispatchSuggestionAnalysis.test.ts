import { DispatchConfigNormalizer, DispatchConfigStore, type RawDispatchConfig, type RunResult } from '@harness/core';
import { afterEach, describe, expect, it } from 'vitest';
import { TestServer } from '../helpers/TestServer.js';

const RAW: RawDispatchConfig = {
  ...new DispatchConfigNormalizer().example(),
  profiles: { standard: { model: 'opus', maxTurns: 30, timeoutMinutes: 60 }, gross: { model: 'opus', maxTurns: 80, timeoutMinutes: 120 } },
  defaultProfile: 'standard', escalate: true, escalation: ['standard', 'gross'], rules: [],
  research: { auto: false, profile: null }, planning: { profile: null },
};

// Antwort von `claude -p --output-format json` mit diesem Text als Ergebnis
const reply = (result: string, cost = 0.04): RunResult =>
  ({ code: 0, timedOut: false, output: JSON.stringify({ type: 'result', subtype: 'success', is_error: false, result, total_cost_usd: cost }) });

describe('Claude-Analyse der Optimierungs-Vorschläge', () => {
  let server: TestServer;
  afterEach(() => server.dispose());

  // Startet einen Server mit Board und diesem Claude-Ersatz; vor erneutem Start im selben Test erst `server.dispose()`
  async function start(run: () => Promise<RunResult>) {
    ({ server } = await TestServer.startWithBoard({ suggestionClaude: { run } }));
    await new DispatchConfigStore(server.home).write(RAW);
  }

  it('übernimmt gültige Claude-Vorschläge, macht ungültige zu Hinweisen und merkt Zeitpunkt und Kosten', async () => {
    await start(async () => reply(JSON.stringify([
      { title: 'Mehr Zeit für gross', reason: 'weil', target: { profile: 'gross' }, patch: [{ path: 'profiles.gross.timeoutMinutes', from: 120, to: 180 }] },
      { title: 'Befehl ändern', reason: 'x', patch: [{ path: 'claude.command', from: 'claude', to: 'rm' }] },
      { title: 'Unsinniger Wert', reason: 'y', patch: [{ path: 'profiles.gross.maxTurns', from: 80, to: -5 }] },
      { reason: 'ohne Titel – unbrauchbar' },
    ])));
    const res = await server.post('/api/dispatch/suggestions/analyze');
    expect(res.status).toBe(200);
    const claude = res.json.suggestions.filter((s: { source: string }) => s.source === 'claude');
    expect(claude.map((s: { title: string; patch: unknown }) => [s.title, s.patch === null])).toEqual([
      ['Mehr Zeit für gross', false], ['Befehl ändern', true], ['Unsinniger Wert', true],
    ]);
    expect(res.json.analysis).toMatchObject({ costUsd: 0.04, stale: false, count: 3, skipped: 1 });
  });

  it('merkt auch ein leeres Ergebnis mit Zeitpunkt und Kosten', async () => {
    await start(async () => reply('[]'));
    const res = await server.post('/api/dispatch/suggestions/analyze');
    expect(res.status).toBe(200);
    expect(res.json.analysis).toMatchObject({ costUsd: 0.04, stale: false, count: 0, skipped: 0 });
    expect(typeof res.json.analysis.at).toBe('string');
  });

  it('kennzeichnet ein Ergebnis zu einer älteren Einstellung als veraltet und nimmt ihm die Änderungen', async () => {
    await start(async () => reply(JSON.stringify([{ title: 'A', reason: 'r', patch: [{ path: 'profiles.gross.timeoutMinutes', from: 120, to: 180 }] }])));
    await server.post('/api/dispatch/suggestions/analyze');
    await new DispatchConfigStore(server.home).write({ ...RAW, defaultProfile: 'gross' });
    const list = (await server.get('/api/dispatch/suggestions')).json;
    expect(list.analysis.stale).toBe(true);
    expect(list.suggestions.find((s: { title: string }) => s.title === 'A').patch).toBeNull();
  });

  it('meldet fehlendes Claude, Zeitlimit und unlesbare Antworten als Fehler', async () => {
    await start(async () => ({ code: 127, timedOut: false, output: 'spawn claude ENOENT' }));
    const missing = await server.post('/api/dispatch/suggestions/analyze');
    expect(missing.status).toBe(422);
    expect(missing.json).toMatchObject({ code: 'analysis-failed', message: expect.stringContaining('nicht gefunden') });
    await server.dispose();
    await start(async () => ({ code: null, timedOut: true, output: '' }));
    expect((await server.post('/api/dispatch/suggestions/analyze')).json.message).toContain('5 Minuten');
    await server.dispose();
    await start(async () => reply('Ich schlage vor, mehr Schritte zu geben.'));
    expect((await server.post('/api/dispatch/suggestions/analyze')).json.message).toContain('nicht lesbar');
  });

  it('lässt nur eine Analyse gleichzeitig laufen', async () => {
    let finish: (r: RunResult) => void = () => {};
    await start(() => new Promise<RunResult>((resolve) => { finish = resolve; }));
    const first = server.post('/api/dispatch/suggestions/analyze');
    await new Promise((r) => setTimeout(r, 50));
    const second = await server.post('/api/dispatch/suggestions/analyze');
    expect(second.status).toBe(409);
    expect(second.json.details).toEqual({ reason: 'analysis-running' });
    finish(reply('[]'));
    expect((await first).status).toBe(200);
  });
});
