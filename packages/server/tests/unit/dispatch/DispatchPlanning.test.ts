import { existsSync } from 'node:fs';
import { afterEach, describe, expect, it } from 'vitest';
import { DISPATCH_MARKERS, PLANNING_DENIED_TOOLS, PLANNING_TOOLS, type BoardQuestionActor } from '@harness/core';
import { DispatchFixture } from '../../helpers/DispatchFixture.js';
import { MEMORY_USER } from '../../helpers/MemoryBoardBackend.js';

const HUMAN: BoardQuestionActor = { userId: MEMORY_USER.id, name: 'Florian', kind: 'human', session: null };

// Legt ein Ticket im Backlog mit Planungsbedarf an und liefert die ID
async function backlog(f: DispatchFixture, title = 'Rabatte'): Promise<string> {
  return (await f.services.tickets.create(f.boardId, { title, needsPlanning: true }, f.backend.actor())).id;
}

// Speichert einen Plan, als hätte der Planungs-Agent `harness board plan-json` aufgerufen
async function planAsAgent(f: DispatchFixture, id: string): Promise<void> {
  await f.services.planning.plan(id, { plan: '## Plan\nUmsetzen\n\n## Aufwand\nklein – eine Datei', handbook: 'npm test', diagram: 'graph TD; A-->B' },
    { userId: MEMORY_USER.id, name: `planner-${id}` });
}

describe('DispatchService – Planen (von Hand)', () => {
  let f: DispatchFixture;
  afterEach(() => f?.dispose());

  it('startet die Planung im losgelösten Worktree, eingeschränkt, mit Planungs-Claim und HARNESS_TICKET', async () => {
    f = await DispatchFixture.create({
      enabled: false, window: { from: '00:00', to: '00:01' },
      claude: { permissionMode: 'bypassPermissions', extraArgs: ['--dangerously-skip-permissions'], allowedTools: ['Bash(*)'] },
    });
    const id = await backlog(f);
    const info = await f.service.startPlanning(id.toLowerCase());
    expect(info).toMatchObject({ kind: 'plan', ticketId: id, title: 'Rabatte', profile: 'standard' });
    expect(info.log).toMatch(new RegExp(`^${id}-\\d{8}-\\d{6}-plan\\.log$`));
    const run = await f.run(id);
    expect(run).toMatchObject({ kind: 'plan', agent: `planner-${id}`, branch: '', plannedBefore: null, repoRoot: expect.any(String) });
    expect(run.cwd).toMatch(new RegExp(`-worktrees/plan-${id}$`));
    expect(existsSync(run.cwd)).toBe(true);
    expect(run.args[run.args.indexOf('--permission-mode') + 1]).toBe('default');
    expect(run.args).not.toContain('--dangerously-skip-permissions');
    expect(run.args.slice(run.args.indexOf('--disallowedTools') + 1, run.args.indexOf('--allowedTools'))).toEqual(PLANNING_DENIED_TOOLS);
    expect(run.args.slice(run.args.indexOf('--allowedTools') + 1)).toEqual(PLANNING_TOOLS);
    expect(run.prompt).toContain(`harness board plan-json ${id} --file plan.json --by planner-${id}`);
    expect(run.prompt).toContain('## Aufwand');
    expect(f.launcher.launched[0].env.HARNESS_TICKET).toBe(id);
    expect((await f.services.tickets.get(id)).planningClaimBy).toBe(`planner-${id}`);
    const status = await f.service.status(f.boardId);
    expect(status).toMatchObject({ active: 0, planning: 1 });
    expect((await f.service.planningStatus(id)).run).toMatchObject({ ticketId: id, kind: 'plan' });
  });

  it('lehnt ab: laufende Planung, falscher Status, offene Frage, Pause, fremdes Board oder Board ohne Repo', async () => {
    f = await DispatchFixture.create();
    const id = await backlog(f);
    await f.service.startPlanning(id);
    await expect(f.service.startPlanning(id)).rejects.toMatchObject({ code: 'plan-running' });
    const done = await backlog(f, 'Fertig');
    await f.services.tickets.update(done, { status: 'in_progress' }, f.backend.actor());
    await expect(f.service.startPlanning(done)).rejects.toMatchObject({ code: 'plan-not-possible' });
    const asked = await backlog(f, 'Mit Frage');
    await f.services.questions.create('ARC', { title: 'Wie?', ticketIds: [asked] }, HUMAN);
    await expect(f.service.startPlanning(asked)).rejects.toMatchObject({ code: 'plan-waiting-question' });
    const later = await backlog(f, 'Später');
    await f.service.pause(30);
    await expect(f.service.startPlanning(later)).rejects.toMatchObject({ code: 'dispatch-paused' });
    await f.service.resume();
    const noRepo = await f.services.boards.create('ARC', 'Ohne Repo', f.backend.actor());
    const elsewhere = (await f.services.tickets.create(noRepo.id, { title: 'Woanders' }, f.backend.actor())).id;
    await expect(f.service.startPlanning(elsewhere)).rejects.toMatchObject({ code: 'plan-no-repo' });
    expect((await f.services.tickets.get(elsewhere)).planningClaimBy).toBeNull();
  });

  it('Planungen zählen nicht zum WIP, das geplante Ticket startet aber nicht, solange seine Planung läuft', async () => {
    f = await DispatchFixture.create({ wip: 1 });
    const planned = await f.ready('Schon geplant');
    await f.service.startPlanning(planned);
    const report = await f.service.tick();
    expect(report.started).toEqual([]);
    const other = await f.ready('Anderes');
    expect((await f.service.tick()).started).toEqual([other]);
  });
});

describe('DispatchService – Planung auswerten', () => {
  let f: DispatchFixture;
  afterEach(() => f?.dispose());

  it('Plan des Agents gespeichert: planned, Worktree entfernt, kein Kommentar', async () => {
    f = await DispatchFixture.create({ enabled: false });
    const id = await backlog(f);
    await f.service.startPlanning(id);
    const run = await f.run(id);
    await planAsAgent(f, id);
    await f.end(id);
    const report = await f.service.tick();
    expect(report.evaluated).toEqual([{ ticketId: id, outcome: 'planned' }]);
    expect(existsSync(run.cwd)).toBe(false);
    const ticket = await f.services.tickets.get(id);
    expect(ticket).toMatchObject({ status: 'planned', needsPlanning: false, planningClaimBy: null, plannedBy: `planner-${id}` });
    expect(await f.comments(id)).toEqual([]);
  });

  it('neue offene Frage: question, Claim frei, kein Kommentar', async () => {
    f = await DispatchFixture.create({ enabled: false });
    const id = await backlog(f);
    await f.service.startPlanning(id);
    await f.services.questions.create('ARC', { title: 'Brutto oder netto?', ticketIds: [id] }, { ...HUMAN, name: `planner-${id}`, kind: 'session' });
    await f.end(id);
    expect((await f.service.tick()).evaluated).toEqual([{ ticketId: id, outcome: 'question' }]);
    expect((await f.services.tickets.get(id)).planningClaimBy).toBeNull();
    expect(await f.comments(id)).toEqual([]);
  });

  it('ohne Plan und ohne Frage: Kommentar „Planung ohne Ergebnis“ mit Protokoll, Claim frei, kein doppelter Kommentar', async () => {
    f = await DispatchFixture.create({ enabled: false });
    const id = await backlog(f);
    await f.service.startPlanning(id);
    const run = await f.run(id);
    await f.end(id);
    expect((await f.service.tick()).evaluated).toEqual([{ ticketId: id, outcome: 'empty' }]);
    const comments = await f.comments(id);
    expect(comments).toHaveLength(1);
    expect(comments[0]).toContain(DISPATCH_MARKERS.PLAN_EMPTY);
    expect(comments[0]).toContain(`${run.id}.log`);
    const ticket = await f.services.tickets.get(id);
    expect(ticket).toMatchObject({ status: 'backlog', needsPlanning: true, planningClaimBy: null });
    expect(existsSync(run.cwd)).toBe(false);
  });

  it('Nutzungslimit: Pause und Kommentar', async () => {
    f = await DispatchFixture.create({ enabled: false });
    const id = await backlog(f);
    await f.service.startPlanning(id);
    await f.end(id, 'Claude usage limit reached', 1);
    expect((await f.service.tick()).evaluated).toEqual([{ ticketId: id, outcome: 'limit' }]);
    expect((await f.service.stores.state.read()).pausedUntil).not.toBeNull();
    expect((await f.comments(id))[0]).toContain('Nutzungslimit erreicht');
  });

  it('ein erneuter Plan an einem schon geplanten Ticket zählt erst mit neuerem plannedAt', async () => {
    f = await DispatchFixture.create({ enabled: false });
    const id = await backlog(f);
    await planAsAgent(f, id);
    await new Promise((done) => setTimeout(done, 5));
    await f.service.startPlanning(id);
    expect((await f.run(id)).plannedBefore).toBe((await f.services.tickets.get(id)).plannedAt);
    await f.end(id);
    expect((await f.service.tick()).evaluated).toEqual([{ ticketId: id, outcome: 'empty' }]);
  });
});
