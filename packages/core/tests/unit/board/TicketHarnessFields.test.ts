import { describe, expect, it } from 'vitest';
import { BoardFixture } from './helpers/BoardFixture.js';
import { BoardViewService } from '../../../src/board/BoardViewService.js';

const STEPS = [{ text: 'Test schreiben', done: false, detail: 'Erwartet: rot' }, { text: 'Umsetzen', done: false }];

describe('Harness-Felder am Ticket', () => {
  it('legt ein Ticket mit Spec, Bereichen, Schnittstellen und Schritten an', async () => {
    const f = await BoardFixture.create();
    const t = await f.tickets.create(f.boardId, {
      title: 'Login', spec: '001-login', specTask: 'T001', touches: [' src/login ', ''], consumes: ['GET /a'], produces: ['<Login>'], steps: STEPS,
    }, f.actor);
    expect(t).toMatchObject({ spec: '001-login', specTask: 'T001', branch: null, worktree: null, touches: ['src/login'], stepsDone: 0, stepsTotal: 2, sessions: [] });
    const detail = await f.tickets.get('ARC-0001');
    expect(detail.steps).toEqual(STEPS);
    expect(detail).toMatchObject({ consumes: ['GET /a'], produces: ['<Login>'] });
  });

  it('ändert Branch, Worktree und Bereiche mit Verlauf und ersetzt Schritte nur bei Änderung', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A', steps: STEPS }, f.actor);
    await f.tickets.update('ARC-0001', { branch: 'ticket/ARC-0001-a', worktree: 'C:/w/ARC-0001', touches: ['src'] }, f.actor);
    const history = (await f.tickets.get('ARC-0001')).history.map((h) => [h.field, h.from, h.to]);
    expect(history).toContainEqual(['branch', null, 'ticket/ARC-0001-a']);
    expect(history).toContainEqual(['worktree', null, 'C:/w/ARC-0001']);
    expect(history).toContainEqual(['touches', '[]', '["src"]']);

    await f.tickets.update('ARC-0001', { steps: [...STEPS, { text: 'Committen', done: false }] }, f.actor);
    const afterSteps = await f.tickets.get('ARC-0001');
    expect(afterSteps.stepsTotal).toBe(3);
    expect(afterSteps.history.at(-1)).toMatchObject({ field: 'steps', from: '2 Schritte', to: '3 Schritte' });

    await f.tickets.update('ARC-0001', { steps: afterSteps.steps, title: 'A2' }, f.actor);
    expect((await f.tickets.get('ARC-0001')).history.filter((h) => h.field === 'steps')).toHaveLength(1);
    await f.tickets.update('ARC-0001', { branch: null }, f.actor);
    expect((await f.tickets.get('ARC-0001')).branch).toBeNull();
  });

  it('prüft Schritte und Längen', async () => {
    const f = await BoardFixture.create();
    await expect(f.tickets.create(f.boardId, { title: 'A', steps: [{ text: '  ', done: false }] }, f.actor)).rejects.toMatchObject({ code: 'step-empty' });
    await expect(f.tickets.create(f.boardId, { title: 'A', spec: 'x'.repeat(201) }, f.actor)).rejects.toMatchObject({ code: 'value-too-long' });
    await expect(f.tickets.create(f.boardId, { title: 'A', steps: [{ text: 'x'.repeat(1001), done: false }] }, f.actor)).rejects.toMatchObject({ code: 'value-too-long' });
    await f.tickets.create(f.boardId, { title: 'B' }, f.actor);
    await expect(f.tickets.update('ARC-0001', { worktree: 'w'.repeat(501) }, f.actor)).rejects.toMatchObject({ code: 'value-too-long' });
  });

  it('hakt Schritte ab und zurück, erhöht dabei die Version und schreibt den Verlauf', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A', steps: STEPS }, f.actor);
    const done = await f.tickets.setStep('ARC-0001', 1, true, f.actor);
    expect(done).toMatchObject({ stepsDone: 1, stepsTotal: 2, rev: 2 });
    expect(done.steps[0].done).toBe(true);
    expect(done.history.at(-1)).toMatchObject({ field: 'step', from: '1: offen', to: '1: erledigt', note: 'Test schreiben' });
    expect((await f.tickets.setStep('ARC-0001', 1, true, f.actor)).rev).toBe(2);
    expect((await f.tickets.setStep('ARC-0001', 1, false, f.actor))).toMatchObject({ stepsDone: 0, rev: 3 });
    await expect(f.tickets.setStep('ARC-0001', 3, true, f.actor)).rejects.toMatchObject({ code: 'step-not-found', message: expect.stringContaining('1 bis 2') });
    await f.tickets.create(f.boardId, { title: 'Ohne' }, f.actor);
    await expect(f.tickets.setStep('ARC-0002', 1, true, f.actor)).rejects.toMatchObject({ code: 'step-not-found', message: expect.stringContaining('keine Schritte') });
  });

  it('verknüpft Sessions über claim und linkSession und findet die Tickets einer Session', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    await f.tickets.create(f.boardId, { title: 'B' }, f.actor);
    const claimed = await f.tickets.claim('ARC-0001', 'claude', f.actor, { sessionId: 's-1' });
    expect(claimed.sessions).toEqual(['s-1']);
    expect(await f.tickets.linkSession('ARC-0002', 's-1')).toBe(true);
    expect(await f.tickets.linkSession('arc-2', 's-1')).toBe(false);
    expect((await f.tickets.get('ARC-0002')).rev).toBe(1);
    expect(await f.tickets.ticketsOfSession('s-1')).toEqual(['ARC-0001', 'ARC-0002']);
    await expect(f.tickets.linkSession('ARC-0002', ' ')).rejects.toMatchObject({ code: 'session-invalid' });
    await expect(f.tickets.linkSession('ARC-0002', 's'.repeat(101))).rejects.toMatchObject({ code: 'value-too-long' });
  });

  it('prüft vor dem Start dieselben Sperren wie In Arbeit; force übergeht offene Voraussetzungen', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'Basis' }, f.actor);
    await f.tickets.create(f.boardId, { title: 'Folge', dependsOn: ['ARC-0001'] }, f.actor);
    await expect(f.tickets.checkStart('ARC-0002')).rejects.toMatchObject({ code: 'dependency-open' });
    expect((await f.tickets.checkStart('ARC-0002', { force: true })).id).toBe('ARC-0002');
    expect((await f.tickets.checkStart('ARC-0001')).status).toBe('backlog');
    await f.questions.create('ARC', { title: 'Offen?', ticketIds: ['ARC-0001'] }, f.agent);
    await expect(f.tickets.checkStart('ARC-0001')).rejects.toMatchObject({ code: 'question-open' });
  });

  it('liefert Kurzangaben zu bekannten Tickets und übergeht unbekannte', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A' }, f.actor);
    await f.tickets.create(f.boardId, { title: 'B', status: 'planned' }, f.actor);
    expect(await f.tickets.summaries(['ARC-0002', 'ARC-0099', 'T-0001', 'kaputt', 'ARC-0001'])).toEqual([
      { id: 'ARC-0002', title: 'B', status: 'planned', boardId: f.boardId },
      { id: 'ARC-0001', title: 'A', status: 'backlog', boardId: f.boardId },
    ]);
    expect(await f.tickets.summaries([])).toEqual([]);
  });

  it('zeigt Fortschritt und Sessions auf den Karten des Boards', async () => {
    const f = await BoardFixture.create();
    await f.tickets.create(f.boardId, { title: 'A', steps: STEPS }, f.actor);
    await f.tickets.setStep('ARC-0001', 2, true, f.actor);
    await f.tickets.linkSession('ARC-0001', 's-7');
    const view = await new BoardViewService(f.db, f.clock).get(f.boardId);
    expect(view.columns.find((c) => c.id === 'backlog')!.tickets[0]).toMatchObject({ stepsDone: 1, stepsTotal: 2, sessions: ['s-7'] });
  });

  it('legt ein Ticket mit Zielbranch an und ändert ihn vor dem Start mit Verlauf', async () => {
    const f = await BoardFixture.create();
    expect(await f.tickets.create(f.boardId, { title: 'A', targetBranch: ' release/1 ' }, f.actor)).toMatchObject({ targetBranch: 'release/1' });
    await f.tickets.update('ARC-0001', { targetBranch: 'release/2' }, f.actor);
    const t = await f.tickets.get('ARC-0001');
    expect(t.targetBranch).toBe('release/2');
    expect(t.history.map((h) => [h.field, h.from, h.to])).toContainEqual(['targetBranch', 'release/1', 'release/2']);
    await f.tickets.update('ARC-0001', { targetBranch: '' }, f.actor);
    expect((await f.tickets.get('ARC-0001')).targetBranch).toBeNull();
  });

  it('lehnt ungültige Zielbranches und Änderungen nach dem Start ab', async () => {
    const f = await BoardFixture.create();
    await expect(f.tickets.create(f.boardId, { title: 'A', targetBranch: 'ticket/ARC-0009-x' }, f.actor)).rejects.toMatchObject({ code: 'target-branch-invalid' });
    await f.tickets.create(f.boardId, { title: 'B', targetBranch: 'release/1' }, f.actor);
    await f.tickets.update('ARC-0001', { branch: 'ticket/ARC-0001-b', worktree: 'C:/w/ARC-0001' }, f.actor);
    await expect(f.tickets.update('ARC-0001', { targetBranch: 'release/2' }, f.actor)).rejects.toMatchObject({ code: 'target-branch-locked' });
    // gleicher Wert ist keine Änderung und bleibt erlaubt
    await f.tickets.update('ARC-0001', { targetBranch: 'release/1', title: 'B2' }, f.actor);
  });
});
