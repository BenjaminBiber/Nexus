import { afterEach, describe, expect, it, vi } from 'vitest';
import type { NotifiableBoardQuestion, ProjectState } from '@harness/core';
import type { BoardRuntimeHolder } from '../../src/dbboard/BoardRuntimeHolder.js';
import { DbQuestionWatch } from '../../src/notifications/DbQuestionWatch.js';
import type { QuestionChange } from '../../src/notifications/NotificationService.js';

// Frage, wie sie die Board-Datenbank für Meldungen liefert
const question = (id: string, extra: Partial<NotifiableBoardQuestion> = {}): NotifiableBoardQuestion => ({
  id, title: 'Welcher Speicher?', occasion: null, origin: { kind: 'session' }, research: [], projectKey: 'ARC', projectName: 'Arches', repoPaths: [], ...extra,
});
const project = (id: string, path: string, reachable = true) => ({ id, path, name: id, reachable }) as unknown as ProjectState;

// Watch mit gefälschter Laufzeit (nur notifiable für Benutzer u1) und gesammelten Meldungen
function setup(projects: ProjectState[] = [project('p1', 'C:/A'), project('p2', 'C:\\Projects\\Arches')], intervalMs = 0) {
  let current: NotifiableBoardQuestion[] = [];
  let failing = false;
  let listFailures = 0;
  let failId: string | null = null;
  let gate: Promise<void> | null = null;
  let notifiableCalls = 0;
  const calls: QuestionChange[] = [];
  const backend = {
    actor: () => ({ userId: 'u1', name: 'Florian' }),
    services: { questions: { notifiable: async (owner: string) => { notifiableCalls++; if (gate) await gate; return owner === 'u1' ? current : []; } } },
  };
  const holder = {
    run: async (work: (b: typeof backend) => Promise<unknown>) => {
      if (failing) throw new Error('Board-Datenbank fehlt');
      return work(backend);
    },
  } as unknown as Pick<BoardRuntimeHolder, 'run'>;
  const watch = new DbQuestionWatch(holder, { list: async () => { if (listFailures > 0) { listFailures--; throw new Error('Projektliste fehlt'); } return projects; } },
    { onQuestionChange: async (c) => { if (c.question.id === failId) throw new Error('Toast kaputt'); calls.push(c); } }, intervalMs);
  return {
    watch, calls, set: (q: NotifiableBoardQuestion[]) => (current = q), fail: (on: boolean) => (failing = on),
    failList: (n: number) => (listFailures = n), failSink: (id: string | null) => (failId = id),
    hold: (g: Promise<void> | null) => (gate = g), notifiableCalls: () => notifiableCalls,
  };
}

describe('DbQuestionWatch', () => {
  afterEach(() => vi.useRealTimers());

  it('meldet beim ersten Durchlauf nichts, danach neue Fragen und Recherchen von Agenten mit passender Adresse', async () => {
    const s = setup();
    s.set([question('ARC-F0001')]);
    await s.watch.tick();
    expect(s.calls).toEqual([]);
    s.set([
      question('ARC-F0001', { research: [{ kind: 'session', options: [] }] }),
      question('ARC-F0002', { repoPaths: ['c:/projects/arches/'] }),
      question('ARC-F0003', { origin: { kind: 'human' } }),
    ]);
    await s.watch.tick();
    expect(s.calls.map((c) => [c.kind, c.question.id, c.projectId, c.projectName, c.route])).toEqual([
      ['recherche', 'ARC-F0001', 'p1', 'ARC · Arches', 'questions'],
      ['frage-neu', 'ARC-F0002', 'p2', 'ARC · Arches', 'questions'],
    ]);
    await s.watch.tick();
    expect(s.calls).toHaveLength(2);
  });

  it('bleibt still, wenn die Board-Datenbank fehlt, und meldet danach nichts doppelt', async () => {
    const s = setup();
    s.fail(true);
    await expect(s.watch.tick()).resolves.toBeUndefined();
    s.fail(false);
    await s.watch.tick();
    s.set([question('ARC-F0001')]);
    await s.watch.tick();
    await s.watch.tick();
    expect(s.calls.map((c) => c.question.id)).toEqual(['ARC-F0001']);
  });

  it('nutzt ohne erreichbares Harness-Projekt eine leere Projekt-ID', async () => {
    const s = setup([project('p1', 'C:/A', false)]);
    await s.watch.tick();
    s.set([question('ARC-F0001')]);
    await s.watch.tick();
    expect(s.calls[0].projectId).toBe('');
  });

  it('startet nicht mit Abstand 0', () => {
    const s = setup();
    s.watch.start();
    s.watch.stop();
    expect(s.calls).toEqual([]);
  });

  it('verliert bei einer fehlschlagenden Meldung die übrigen Änderungen nicht', async () => {
    const s = setup();
    await s.watch.tick();
    s.failSink('ARC-F0001');
    s.set([question('ARC-F0001'), question('ARC-F0002'), question('ARC-F0003')]);
    await s.watch.tick();
    expect(s.calls.map((c) => c.question.id)).toEqual(['ARC-F0002', 'ARC-F0003']);
  });

  it('meldet bei fehlgeschlagener Projektliste im nächsten Durchlauf, danach nichts doppelt', async () => {
    const s = setup();
    await s.watch.tick();
    s.set([question('ARC-F0001')]);
    s.failList(1);
    await s.watch.tick();
    expect(s.calls).toEqual([]);
    await s.watch.tick();
    expect(s.calls.map((c) => c.question.id)).toEqual(['ARC-F0001']);
    await s.watch.tick();
    expect(s.calls).toHaveLength(1);
  });

  it('pollt mit positivem Abstand bis stop() und lässt Durchläufe nicht überlappen', async () => {
    vi.useFakeTimers();
    const s = setup(undefined, 1000);
    s.watch.start();
    await vi.advanceTimersByTimeAsync(0);
    expect(s.notifiableCalls()).toBe(1);
    await vi.advanceTimersByTimeAsync(1000);
    expect(s.notifiableCalls()).toBe(2);
    let release!: () => void;
    s.hold(new Promise<void>((r) => (release = r)));
    await vi.advanceTimersByTimeAsync(1000);
    expect(s.notifiableCalls()).toBe(3);
    await vi.advanceTimersByTimeAsync(3000);
    expect(s.notifiableCalls()).toBe(3);
    s.hold(null);
    release();
    await vi.advanceTimersByTimeAsync(1000);
    expect(s.notifiableCalls()).toBe(4);
    s.watch.stop();
    await vi.advanceTimersByTimeAsync(5000);
    expect(s.notifiableCalls()).toBe(4);
  });
});
