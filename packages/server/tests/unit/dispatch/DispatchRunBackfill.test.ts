import { afterEach, describe, expect, it } from 'vitest';
import type { DispatchRun } from '@harness/core';
import { DispatchFixture, OK_OUTPUT } from '../../helpers/DispatchFixture.js';

describe('Backfill aus [dispatch]-Kommentaren', () => {
  let f: DispatchFixture;
  afterEach(() => f?.dispose());

  const projectId = async () => (await f.services.projects.list()).find((p) => p.key === 'ARC')!.id;
  const runs = async (): Promise<DispatchRun[]> => {
    const id = await projectId();
    return f.services.db.transaction((u) => u.dispatchRuns.list({ projectId: id, from: '2000-01-01T00:00:00.000Z', to: '2100-01-01T00:00:00.000Z' }));
  };
  // Vergleichbare Felder eines Laufs
  const shape = (r: DispatchRun) => [r.subjectId, r.attempt, r.profile, r.escalatedFrom, r.requestedModel, r.effort, r.outcome, r.abortCategory];
  // Die Test-Uhr steht still: Läufe mit gleicher Startzeit haben keine feste Reihenfolge
  const shapes = (list: DispatchRun[]) => list.map(shape).sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b)));

  it('rekonstruiert aus den echten Kommentaren des Dispatchers dieselben Läufe, die er selbst erfasst hat', async () => {
    f = await DispatchFixture.create();
    // Ticket 1: Abbruch mit Exit-Code, eskalierter zweiter Versuch ohne Ausgang (letzter Versuch)
    const hard = await f.ready('Schwierig');
    await f.service.tick();
    await f.end(hard, 'Fehlerausgabe', 2);
    await f.service.tick();
    await f.end(hard, OK_OUTPUT, 0);
    await f.service.tick();
    // Ticket 2: Nutzungslimit, danach erledigt
    const ok = await f.ready('Leicht');
    await f.service.tick();
    await f.end(ok, 'Claude usage limit reached', 1);
    await f.service.tick();
    await f.service.resume();
    await f.service.tick();
    await f.services.tickets.move(ok, 'done', f.agent(ok));
    await f.end(ok);
    await f.service.tick();

    const live = (await runs()).filter((r) => r.source === 'dispatcher');
    expect(shapes(live)).toEqual([
      [hard, 1, 'standard', null, 'opus', 'high', 'abort', 'exit-code'],
      [hard, 2, 'schwer', 'standard', 'opus', 'max', 'abort-final', 'no-outcome'],
      [ok, 1, 'standard', null, 'opus', 'high', 'done', null],
      [ok, 1, 'standard', null, 'opus', 'high', 'pause-limit', null],
    ]);

    // Trockenlauf schreibt nichts, der echte Lauf legt dieselben Läufe mit Herkunft „comments“ an, ein zweiter nichts mehr
    const dry = await f.services.dispatchRuns.backfill(await projectId(), { dryRun: true });
    expect(dry.found).toBe(4);
    expect((await runs()).filter((r) => r.source === 'comments')).toEqual([]);
    const first = await f.services.dispatchRuns.backfill(await projectId());
    expect(shapes(first.created)).toEqual(shapes(live));
    expect(first.created.every((r) => r.id.startsWith('c') && r.costUsd === null && r.model === null)).toBe(true);
    expect((await f.services.dispatchRuns.backfill(await projectId())).created).toEqual([]);
    expect((await runs()).filter((r) => r.source === 'comments')).toHaveLength(4);
  });

  it('lässt einen noch laufenden Lauf aus', async () => {
    f = await DispatchFixture.create();
    await f.ready('Läuft');
    await f.service.tick();
    expect((await f.services.dispatchRuns.backfill(await projectId())).found).toBe(0);
  });
});
