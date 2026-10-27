import { afterEach, describe, expect, it, vi } from 'vitest';
import { DispatchConfigStore, HarnessError, PLANNING_TOOLS, type BoardQuestionActor } from '@harness/core';
import { PlanRunner } from '../../../src/dispatch/PlanRunner.js';
import { DispatchFixture } from '../../helpers/DispatchFixture.js';
import { MEMORY_CONFIG, MEMORY_USER } from '../../helpers/MemoryBoardBackend.js';

const HUMAN: BoardQuestionActor = { userId: MEMORY_USER.id, name: 'Florian', kind: 'human', session: null };

// Ändert die Einstellung des Dispatchers
async function configure(f: DispatchFixture, patch: Record<string, unknown>): Promise<void> {
  const store = new DispatchConfigStore(f.home);
  await store.write({ ...(await store.read()).raw, ...patch });
}

// Wartet kurz, damit Zeitstempel davor und danach sicher verschieden sind
function tick(): Promise<void> {
  return new Promise((done) => setTimeout(done, 5));
}

// Schaltet die Planungs-Automatik ein und lässt einen Tick den Zeitpunkt „ab jetzt“ merken
async function autoOn(f: DispatchFixture, planning: Record<string, unknown> = {}): Promise<void> {
  await tick();
  f.now = new Date();
  await configure(f, { planning: { auto: true, ...planning } });
  await f.service.tick();
  await tick();
}

// Legt ein Ticket mit Planungsbedarf an und liefert die ID
async function needsPlanning(f: DispatchFixture, title: string, boardId?: number): Promise<string> {
  const created = await f.services.tickets.create(boardId ?? f.boardId, { title, needsPlanning: true }, f.backend.actor());
  return created.id;
}

describe('DispatchService – Planungs-Automatik', () => {
  let f: DispatchFixture;
  afterEach(() => {
    vi.restoreAllMocks();
    f?.dispose();
  });

  it('aus: nichts; an: Planung im losgelösten Worktree, nur lesend, je Ticket einmal', async () => {
    f = await DispatchFixture.create();
    const early = await needsPlanning(f, 'Vor dem Einschalten');
    expect((await f.service.tick()).planned).toEqual([]);
    expect((await f.service.stores.state.read()).planningAuto.since).toBeNull();

    await autoOn(f);
    expect((await f.service.stores.state.read()).planningAuto.since).toBe(f.now.toISOString());
    const ticket = await needsPlanning(f, 'Rabatte berechnen');
    const report = await f.service.tick();
    expect(report.planned).toEqual([ticket]);
    const run = await f.run(ticket);
    expect(run).toMatchObject({ kind: 'plan', agent: `planner-${ticket}`, branch: '' });
    expect(run.cwd).toMatch(new RegExp(`-worktrees/plan-${ticket}$`));
    expect(run.args.slice(run.args.indexOf('--allowedTools') + 1)).toEqual(PLANNING_TOOLS);
    expect((await f.service.stores.state.read()).planning[ticket]).toMatchObject({ count: 1, autoAt: expect.any(String) });
    // Kein zweiter Anstoß, auch nicht nach dem Lauf; ältere Tickets bleiben außen vor
    expect((await f.service.tick()).planned).toEqual([]);
    expect((await f.service.stores.state.read()).planning[early]).toBeUndefined();
  });

  it('startet erst, wenn im WIP der Tickets Platz ist', async () => {
    f = await DispatchFixture.create({ wip: 1 });
    await autoOn(f, { wip: 1 });
    const work = await f.ready('Umsetzen');
    const ticket = await needsPlanning(f, 'Planen');
    const first = await f.service.tick();
    expect(first.started).toEqual([work]);
    expect(first.planned).toEqual([]);
    // Der laufende Umsetzungslauf belegt den einzigen Platz; mit einem Platz mehr wird geplant
    expect((await f.service.tick()).planned).toEqual([]);
    await configure(f, { wip: 2, planning: { auto: true, wip: 1 } });
    expect((await f.service.tick()).planned).toEqual([ticket]);
  });

  it('planning.wip begrenzt gleichzeitige Planungen, ein Tageslimit gibt es nicht', async () => {
    f = await DispatchFixture.create({ enabled: false });
    await autoOn(f, { wip: 2 });
    const ids: string[] = [];
    for (const title of ['Eins', 'Zwei', 'Drei', 'Vier']) ids.push(await needsPlanning(f, title));
    expect((await f.service.tick()).planned).toEqual(ids.slice(0, 2));
    expect((await f.service.tick()).planned).toEqual([]);
    for (const id of ids.slice(0, 2)) await f.end(id);
    // Derselbe Tick wertet die beendeten Läufe aus und stößt die nächsten Kandidaten an
    expect((await f.service.tick()).planned).toEqual(ids.slice(2));
  });

  it('überspringt Tickets mit offener Frage, fremde Boards und Boards ohne Repo-Pfad', async () => {
    f = await DispatchFixture.create({ enabled: false });
    const noRepo = await f.services.boards.create('ARC', 'Ohne Repo', f.backend.actor());
    await autoOn(f, { wip: 5 });
    const waiting = await needsPlanning(f, 'Mit Frage');
    await f.services.questions.create('ARC', { title: 'Brutto oder netto?', ticketIds: [waiting] }, HUMAN);
    const homeless = await needsPlanning(f, 'Ohne Repo', noRepo.id);
    const fine = await needsPlanning(f, 'Sauber');
    expect((await f.service.tick()).planned).toEqual([fine]);
    const state = await f.service.stores.state.read();
    expect(state.planning[waiting]).toBeUndefined();
    expect(state.planning[homeless]).toBeUndefined();
  });

  it('plant ein Ticket nach einem Lauf ohne Ergebnis nicht erneut automatisch', async () => {
    f = await DispatchFixture.create({ enabled: false });
    await autoOn(f, { wip: 5 });
    const ticket = await needsPlanning(f, 'Bleibt offen');
    expect((await f.service.tick()).planned).toEqual([ticket]);
    await f.end(ticket, '', 1);
    const after = await f.service.tick();
    expect(after.evaluated).toEqual([{ ticketId: ticket, outcome: 'empty' }]);
    expect((await f.services.tickets.get(ticket)).needsPlanning).toBe(true);
    expect((await f.service.tick()).planned).toEqual([]);
  });

  it('respektiert Pause und Zeitfenster, nicht aber den Schalter des Dispatchers', async () => {
    f = await DispatchFixture.create({ enabled: false });
    await autoOn(f, { wip: 5 });
    const ticket = await needsPlanning(f, 'Planen');
    await f.service.pause(10);
    expect((await f.service.tick()).planned).toEqual([]);
    await f.service.resume();
    await configure(f, { window: { from: '00:00', to: '00:01' }, planning: { auto: true, wip: 5 } });
    f.now = new Date(f.now.getFullYear(), f.now.getMonth(), f.now.getDate(), 12, 0, 0);
    expect((await f.service.tick()).planned).toEqual([]);
    await configure(f, { window: null, planning: { auto: true, wip: 5 } });
    const report = await f.service.tick();
    expect(report.idle).toBe('Dispatcher ist ausgeschaltet');
    expect(report.planned).toEqual([ticket]);
  });

  it('Fehler des Tickets merken es als erledigt; die übrigen werden weiter bedient', async () => {
    f = await DispatchFixture.create({ enabled: false });
    await autoOn(f, { wip: 5 });
    const broken = await needsPlanning(f, 'Kaputt');
    const fine = await needsPlanning(f, 'Gut');
    const original = PlanRunner.prototype.start;
    vi.spyOn(PlanRunner.prototype, 'start').mockImplementation(function (this: PlanRunner, id: string, mode?: 'manual' | 'auto') {
      if (id === broken) return Promise.reject(new HarnessError('plan-no-repo', `${id} liegt nicht auf einem eigenen Board mit Repo-Pfad`));
      return original.call(this, id, mode);
    });
    const report = await f.service.tick();
    expect(report.planned).toEqual([fine]);
    expect(report.errors).toEqual([expect.stringContaining(`${broken}: ${broken} liegt nicht auf einem eigenen Board`)]);
    expect((await f.service.stores.state.read()).planning[broken].autoAt).not.toBeNull();
  });

  it('Umgebungsfehler: Abbruch ohne Merken, einmal je Fehlertext gemeldet, Neuversuch frühestens nach 30 min', async () => {
    f = await DispatchFixture.create({ enabled: false });
    await autoOn(f, { wip: 5 });
    const ticket = await needsPlanning(f, 'Planen');
    vi.spyOn(PlanRunner.prototype, 'start').mockRejectedValue(new Error('Festplatte voll'));
    const first = await f.service.tick();
    expect(first.planned).toEqual([]);
    expect(first.errors).toEqual([expect.stringContaining('Planungs-Automatik ausgesetzt bis')]);
    const state = await f.service.stores.state.read();
    expect(state.planning[ticket]).toBeUndefined();
    expect(state.planningAuto.lastError).toBe('Festplatte voll');
    // Ausgesetzt: kein neuer Versuch, kein zweiter Bericht
    expect((await f.service.tick()).errors).toEqual([]);
    f.advance(31);
    vi.restoreAllMocks();
    expect((await f.service.tick()).planned).toEqual([ticket]);
  });

  it('aus- und wieder einschalten: neuer Zeitpunkt, Tickets aus der Zwischenzeit bleiben außen vor', async () => {
    f = await DispatchFixture.create({ enabled: false });
    await autoOn(f, { wip: 5 });
    await configure(f, { planning: { auto: false } });
    await f.service.tick();
    expect((await f.service.stores.state.read()).planningAuto.since).toBeNull();
    await needsPlanning(f, 'Zwischendurch');
    await autoOn(f, { wip: 5 });
    const fresh = await needsPlanning(f, 'Neu');
    expect((await f.service.tick()).planned).toEqual([fresh]);
  });

  it('Status und Einstellung melden die Automatik', async () => {
    f = await DispatchFixture.create({ enabled: false });
    await autoOn(f, { wip: 2 });
    await needsPlanning(f, 'Planen');
    await f.service.tick();
    const status = await f.service.status(f.boardId);
    expect(status.planning).toBe(1);
    expect(status.planningAuto).toMatchObject({ auto: true, wip: 2, active: 1, error: null });
    expect(status.planningAuto).not.toHaveProperty('maxPerDay');
    expect(status.planningAuto.since).not.toBeNull();
  });
});
