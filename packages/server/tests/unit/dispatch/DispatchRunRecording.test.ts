import { afterEach, describe, expect, it, vi } from 'vitest';
import { DispatchConfigFingerprint, DispatchConfigStore, HarnessError, type DispatchRun } from '@harness/core';
import { DispatchFixture, OK_OUTPUT } from '../../helpers/DispatchFixture.js';

// Ergebnis-Ereignis wie von claude -p --output-format stream-json (gekürzt)
const COST_OUTPUT = [
  JSON.stringify({ type: 'system', subtype: 'init', model: 'opus' }),
  JSON.stringify({
    type: 'result', subtype: 'success', is_error: false, result: 'fertig', num_turns: 17, duration_ms: 240_000, total_cost_usd: 0.42,
    usage: { input_tokens: 12, output_tokens: 3400, cache_read_input_tokens: 50_000, cache_creation_input_tokens: 9000 },
    modelUsage: { 'claude-opus-5-5': { inputTokens: 12, outputTokens: 3400, costUSD: 0.4 }, 'claude-haiku-5-5': { inputTokens: 1, outputTokens: 2, costUSD: 0.02 } },
  }),
].join('\n');

// Ergebnis mit zwei abgelehnten Aufrufen desselben Werkzeugs und einem anderen
const DENIED_OUTPUT = JSON.stringify({
  type: 'result', subtype: 'success', is_error: false, result: 'fertig',
  permission_denials: [
    { tool_name: 'Bash', tool_input: { command: 'git push' } },
    { tool_name: 'Bash', tool_input: { command: 'rm -rf /' } },
    { tool_name: 'Edit' },
  ],
});

describe('Dispatcher erfasst Läufe für die Auswertung', () => {
  let f: DispatchFixture;
  afterEach(() => {
    vi.restoreAllMocks();
    f?.dispose();
  });

  // Alle erfassten Läufe des Projekts ARC
  async function runs(): Promise<DispatchRun[]> {
    const projectId = (await f.services.projects.list()).find((p) => p.key === 'ARC')!.id;
    return f.services.db.transaction((u) => u.dispatchRuns.list({ projectId, from: '2000-01-01T00:00:00.000Z', to: '2100-01-01T00:00:00.000Z' }));
  }

  it('erledigter Lauf: eine Zeile mit Kosten, Tokens, Modell, Profil und Versuch', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Fertig');
    await f.service.tick();
    const record = await f.run(id);
    await f.services.tickets.move(id, 'done', f.agent(id));
    await f.end(id, COST_OUTPUT);
    await f.service.tick();
    const [row, ...rest] = await runs();
    expect(rest).toEqual([]);
    expect(row).toMatchObject({
      id: expect.stringMatching(new RegExp(`^${record.id}-[0-9a-f]{8}$`)), kind: 'work', subjectId: id, boardId: f.boardId, agent: `junior-${id}`, attempt: 1, profile: 'standard',
      escalatedFrom: null, requestedModel: 'opus', model: 'claude-opus-5-5', effort: 'high', outcome: 'done', abortCategory: null,
      costUsd: 0.42, inputTokens: 12, outputTokens: 3400, cacheReadTokens: 50_000, cacheWriteTokens: 9000, numTurns: 17,
      exitCode: 0, timedOut: false, source: 'dispatcher', startedAt: record.startedAt,
      // Limits und Grund der Profilwahl, wie sie für den Lauf galten (Standardprofil der Beispiel-Einstellung)
      maxTurns: record.run.maxTurns, timeoutMinutes: record.run.timeoutMinutes, profileReason: 'default', ruleIndex: null,
    });
    expect(JSON.parse(row.modelUsage!)).toHaveProperty('claude-haiku-5-5');
    expect(await f.service.stores.runs.keptStats()).toEqual([]);
  });

  it('Abbruch und Eskalation: Kategorie, Basisprofil, letzter Versuch als abort-final', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Schwierig');
    await f.service.tick();
    await f.end(id, 'Fehlerausgabe', 2);
    await f.service.tick();
    await f.end(id, OK_OUTPUT, 0);
    await f.service.tick();
    expect((await runs()).sort((a, b) => a.attempt! - b.attempt!).map((r) => [r.attempt, r.profile, r.escalatedFrom, r.outcome, r.abortCategory, r.costUsd])).toEqual([
      [1, 'standard', null, 'abort', 'exit-code', null],
      [2, 'schwer', 'standard', 'abort-final', 'no-outcome', null],
    ]);
  });

  it('Nutzungslimit: als Pause erfasst, nicht als Abbruch', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Limit');
    await f.service.tick();
    await f.end(id, 'Claude usage limit reached', 1);
    await f.service.tick();
    expect((await runs()).map((r) => r.outcome)).toEqual(['pause-limit']);
  });

  it('gescheiterter Start: Abbruch mit Kategorie start-failed', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Kein Claude');
    f.launcher.fail = new Error('claude fehlt');
    await f.service.tick();
    await f.service.tick();
    expect((await runs()).sort((a, b) => a.attempt! - b.attempt!).map((r) => [r.subjectId, r.outcome, r.abortCategory, r.attempt])).toEqual([
      [id, 'abort', 'start-failed', 1],
      [id, 'abort-final', 'start-failed', 2],
    ]);
    // Auch ohne Lauf stehen Limits und Grund der Profilwahl in der Zeile – der zweite Versuch mit den Limits des eskalierten Profils
    expect((await runs()).sort((a, b) => a.attempt! - b.attempt!).map((r) => [r.maxTurns, r.timeoutMinutes, r.profileReason, r.ruleIndex])).toEqual([
      [80, 90, 'default', null], [150, 120, 'default', null],
    ]);
  });

  it('Lauf eines älteren Stands ohne Grund: profileReason bleibt leer', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Alt');
    await f.service.tick();
    const record = await f.run(id);
    // Lauf-Datei zurückschreiben, wie ein Stand ohne reason/ruleIndex sie geschrieben hätte
    await f.service.stores.runs.create({ ...record, run: { ...record.run, reason: undefined, ruleIndex: undefined } });
    await f.services.tickets.move(id, 'done', f.agent(id));
    await f.end(id, COST_OUTPUT);
    await f.service.tick();
    expect((await runs()).map((r) => [r.profileReason, r.ruleIndex, r.maxTurns])).toEqual([[null, null, 80]]);
  });

  it('Konfig-Version: Lauf trägt den Hash, die Fassung wird genau einmal angelegt', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Konfig');
    await f.service.tick();
    await f.services.tickets.move(id, 'done', f.agent(id));
    await f.end(id, COST_OUTPUT);
    await f.service.tick();
    const cfg = (await new DispatchConfigStore(f.home).read()).config;
    const hash = new DispatchConfigFingerprint().hash(cfg);
    expect((await runs()).map((r) => r.configHash)).toEqual([hash]);

    const versions = () => f.services.db.transaction((u) => u.dispatchRuns.listConfigVersions('2000-01-01T00:00:00.000Z', '2100-01-01T00:00:00.000Z'));
    const [version, ...rest] = await versions();
    expect(rest).toEqual([]);
    expect(version).toMatchObject({ hash, dispatcher: cfg.dispatcherName });
    expect(JSON.parse(version.content).defaultProfile).toBe(cfg.defaultProfile);

    // Zweiter Lauf derselben Einstellung: keine zweite Fassung
    const next = await f.ready('Noch einer');
    await f.service.tick();
    await f.services.tickets.move(next, 'done', f.agent(next));
    await f.end(next, COST_OUTPUT);
    await f.service.tick();
    expect(await versions()).toEqual([version]);
  });

  it('Lauf eines älteren Stands ohne Hash: configHash bleibt leer', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Ohne Hash');
    await f.service.tick();
    const record = await f.run(id);
    await f.service.stores.runs.create({ ...record, run: { ...record.run, configHash: undefined } });
    await f.services.tickets.move(id, 'done', f.agent(id));
    await f.end(id, COST_OUTPUT);
    await f.service.tick();
    expect((await runs()).map((r) => r.configHash)).toEqual([null]);
  });

  it('erfolgreicher Lauf mit abgelehnten Werkzeugaufrufen: deniedCount und eindeutige Werkzeugnamen erfasst', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Abgelehnt');
    await f.service.tick();
    await f.services.tickets.move(id, 'done', f.agent(id));
    await f.end(id, DENIED_OUTPUT);
    await f.service.tick();
    expect((await runs()).map((r) => [r.outcome, r.deniedCount, r.deniedTools])).toEqual([['done', 3, 'Bash, Edit']]);
  });

  it('Ergebnis ohne Ablehnungen: deniedCount 0, deniedTools leer', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Ohne Ablehnung');
    await f.service.tick();
    await f.services.tickets.move(id, 'done', f.agent(id));
    await f.end(id, OK_OUTPUT);
    await f.service.tick();
    expect((await runs()).map((r) => [r.deniedCount, r.deniedTools])).toEqual([[0, null]]);
  });

  it('Ausgabe ohne Ergebnis-Objekt: deniedCount und deniedTools bleiben leer', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Hart beendet');
    await f.service.tick();
    await f.end(id, 'Fehlerausgabe ohne JSON', 1);
    await f.service.tick();
    expect((await runs()).map((r) => [r.deniedCount, r.deniedTools])).toEqual([[null, null]]);
  });

  it('Verbindungsfehler beim Schreiben: Lauf bleibt vorgemerkt, wird nicht erneut ausgewertet und später geschrieben', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Netz');
    await f.service.tick();
    await f.services.tickets.move(id, 'done', f.agent(id));
    await f.end(id, COST_OUTPUT);
    vi.spyOn(f.services.dispatchRuns, 'record').mockRejectedValueOnce(new HarnessError('board-db-unreachable', 'Datenbank weg'));
    const first = await f.service.tick();
    expect(first.evaluated).toEqual([{ ticketId: id, outcome: 'done' }]);
    expect(first.errors).toEqual(['Datenbank weg']);
    expect(await f.service.stores.runs.list()).toEqual([]);
    expect(await f.service.stores.runs.keptStats()).toHaveLength(1);
    expect(await runs()).toEqual([]);
    const second = await f.service.tick();
    expect(second.evaluated).toEqual([]);
    expect((await runs()).map((r) => r.outcome)).toEqual(['done']);
    expect(await f.service.stores.runs.keptStats()).toEqual([]);
  });

  it('fehlende Erweiterung beim Schreiben: Lauf bleibt vorgemerkt und wird nach dem Einspielen geschrieben', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Schema');
    await f.service.tick();
    await f.services.tickets.move(id, 'done', f.agent(id));
    await f.end(id, COST_OUTPUT);
    const outdated = new HarnessError('board-schema-outdated', 'Erweiterung dispatch-run-details fehlt');
    vi.spyOn(f.services.dispatchRuns, 'recordConfigVersion').mockRejectedValueOnce(outdated);
    vi.spyOn(f.services.dispatchRuns, 'record').mockRejectedValueOnce(outdated);
    const first = await f.service.tick();
    expect(first.evaluated).toEqual([{ ticketId: id, outcome: 'done' }]);
    expect(first.errors).toEqual([]);
    expect(await f.service.stores.runs.keptStats()).toHaveLength(1);
    expect(await runs()).toEqual([]);
    await f.service.tick();
    expect((await runs()).map((r) => r.outcome)).toEqual(['done']);
    expect(await f.service.stores.runs.keptStats()).toEqual([]);
  });
});
