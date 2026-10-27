import { DispatchConfigNormalizer, DispatchConfigStore, type RawDispatchConfig } from '@harness/core';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { MemoryBoardBackend } from '../helpers/MemoryBoardBackend.js';
import { TestServer } from '../helpers/TestServer.js';
import { runFixture } from '../../../core/tests/unit/board/helpers/DispatchRunContract.js';

const RAW: RawDispatchConfig = {
  ...new DispatchConfigNormalizer().example(),
  profiles: { standard: { model: 'opus', maxTurns: 30, timeoutMinutes: 60 }, gross: { model: 'opus', maxTurns: 80, timeoutMinutes: 120 } },
  defaultProfile: 'standard', escalate: true, escalation: ['standard', 'gross'], rules: [],
  research: { auto: false, profile: null }, planning: { profile: null },
};

describe('DispatchSuggestionRoutes', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;

  beforeEach(async () => {
    ({ server, backend } = await TestServer.startWithBoard());
    await new DispatchConfigStore(server.home).write(RAW);
    const arc = (await backend.services.projects.create('ARC', 'Arches', backend.actor())).id;
    const oth = (await backend.services.projects.create('OTH', 'Andere', backend.actor())).id;
    // 5 Läufe mit „standard“ in zwei Projekten, 2 davon an den Schritten gescheitert
    for (let i = 0; i < 5; i++) {
      await backend.services.dispatchRuns.record(runFixture({
        id: `r${i}`, subjectId: `T${i}`, projectId: i % 2 ? oth : arc, startedAt: new Date(Date.now() - i * 60_000).toISOString(),
        maxTurns: 30, timeoutMinutes: 60, outcome: i < 2 ? 'abort' : 'done', abortCategory: i < 2 ? 'max-turns' : null,
      }));
    }
  });
  afterEach(() => server.dispose());

  it('wertet Läufe aller Projekte aus und schlägt mehr Schritte vor', async () => {
    const reply = await server.get('/api/dispatch/suggestions');
    expect(reply.status).toBe(200);
    expect(reply.json.basis).toMatchObject({ runs: 5, projects: 2 });
    expect(reply.json.suggestions[0]).toMatchObject({ kind: 'turns-too-low', patch: [{ path: 'profiles.standard.maxTurns', from: 30, to: 45 }] });
    expect(reply.json.insufficient).toEqual([{ profile: 'gross', runs: 0, needed: 5 }]);
    expect(reply.json.analysis).toBeNull();
  });

  it('übernimmt einen Vorschlag in dispatch.yaml; danach fehlt er, bis neue Läufe ihn stützen', async () => {
    const list = (await server.get('/api/dispatch/suggestions')).json;
    const reply = await server.post(`/api/dispatch/suggestions/${list.suggestions[0].id}/apply`, { configHash: list.configHash });
    expect(reply.status).toBe(200);
    expect(reply.json.view.raw.profiles.standard.maxTurns).toBe(45);
    expect((await new DispatchConfigStore(server.home).read()).config.profiles.standard!.maxTurns).toBe(45);
    expect(reply.json.list.suggestions.map((s: { kind: string }) => s.kind)).not.toContain('turns-too-low');
  });

  it('lehnt das Übernehmen mit veralteter Einstellung oder unbekannter ID ab', async () => {
    const list = (await server.get('/api/dispatch/suggestions')).json;
    const stale = await server.post(`/api/dispatch/suggestions/${list.suggestions[0].id}/apply`, { configHash: 'veraltet' });
    expect(stale.status).toBe(409);
    expect(stale.json).toMatchObject({ code: 'conflict', details: { reason: 'config-changed' } });
    const gone = await server.post('/api/dispatch/suggestions/0000000000000000/apply', { configHash: list.configHash });
    expect(gone.status).toBe(404);
    expect(gone.json.code).toBe('suggestion-not-found');
  });

  it('blendet Vorschläge aus und wieder ein', async () => {
    const { suggestions } = (await server.get('/api/dispatch/suggestions')).json;
    const hidden = await server.post(`/api/dispatch/suggestions/${suggestions[0].id}/dismiss`);
    expect(hidden.json.suggestions).toEqual([]);
    expect(hidden.json.dismissed.map((s: { id: string }) => s.id)).toEqual([suggestions[0].id]);
    const back = await server.delete(`/api/dispatch/suggestions/${suggestions[0].id}/dismiss`);
    expect(back.json.suggestions.map((s: { id: string }) => s.id)).toEqual([suggestions[0].id]);
    expect((await server.post('/api/dispatch/suggestions/kaputt!/dismiss')).status).toBe(422);
  });
});
