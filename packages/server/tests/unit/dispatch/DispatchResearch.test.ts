import { existsSync } from 'node:fs';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  BoardConfigStore, DispatchConfigStore, HarnessError, RESEARCH_DENIED_TOOLS, RESEARCH_TOOLS, ResearchWorktree, type BoardQuestionActor,
} from '@harness/core';
import { DispatchContext } from '../../../src/dispatch/DispatchContext.js';
import { ResearchRunner } from '../../../src/dispatch/ResearchRunner.js';
import { DispatchFixture } from '../../helpers/DispatchFixture.js';
import { MEMORY_CONFIG, MEMORY_USER } from '../../helpers/MemoryBoardBackend.js';

const HUMAN: BoardQuestionActor = { userId: MEMORY_USER.id, name: 'Florian', kind: 'human', session: null };
const OTHER_USER = { id: '22222222-2222-2222-2222-222222222222', name: 'Bob' };
const OTHER: BoardQuestionActor = { userId: OTHER_USER.id, name: 'Bob', kind: 'human', session: null };
const RESEARCH = { context: 'Fundstelle src/a.ts:1', options: [{ title: 'Brutto', text: 'immer brutto' }, { title: 'Netto', text: 'immer netto' }] };

// Ändert die Einstellung des Dispatchers
async function configure(f: DispatchFixture, patch: Record<string, unknown>): Promise<void> {
  const store = new DispatchConfigStore(f.home);
  await store.write({ ...(await store.read()).raw, ...patch });
}

// Wartet kurz, damit Zeitstempel davor und danach sicher verschieden sind
function tick(): Promise<void> {
  return new Promise((done) => setTimeout(done, 5));
}

// Schaltet die Automatik ein und lässt einen Tick den Zeitpunkt „ab jetzt“ merken (Uhr des Dispatchers = jetzt)
async function autoOn(f: DispatchFixture, research: Record<string, unknown> = {}): Promise<void> {
  await tick();
  f.now = new Date();
  await configure(f, { research: { auto: true, ...research } });
  await f.service.tick();
  await tick();
}

describe('DispatchService – Recherche starten (von Hand)', () => {
  let f: DispatchFixture;
  afterEach(() => f?.dispose());

  it('startet die Recherche im losgelösten Worktree des Board-Repos, eingeschränkt, ohne HARNESS_TICKET', async () => {
    f = await DispatchFixture.create({
      enabled: false, window: { from: '00:00', to: '00:01' },
      claude: { permissionMode: 'bypassPermissions', extraArgs: ['--dangerously-skip-permissions'], allowedTools: ['Bash(*)'] },
    });
    const q = await f.services.questions.create('ARC', { title: 'Brutto oder netto?' }, HUMAN);
    const info = await f.service.startResearch(q.id.toLowerCase());
    expect(info).toMatchObject({ kind: 'research', ticketId: q.id, questionId: q.id, title: 'Brutto oder netto?', profile: 'standard' });
    expect(info.log).toMatch(new RegExp(`^${q.id}-\\d{8}-\\d{6}-research\\.log$`));
    const run = await f.run(q.id);
    expect(run).toMatchObject({ kind: 'research', agent: `research-${q.id}`, branch: '', researchBefore: 0, repoRoot: expect.any(String) });
    expect(run.cwd).toMatch(new RegExp(`-worktrees/research-${q.id}$`));
    expect(existsSync(run.cwd)).toBe(true);
    expect(f.repo.git('-C', run.cwd, 'rev-parse', '--abbrev-ref', 'HEAD')).toBe('HEAD');
    // Einstellungen (Rechte-Modus, extraArgs, allowedTools) heben die Einschränkung nicht auf
    expect(run.args[run.args.indexOf('--permission-mode') + 1]).toBe('default');
    expect(run.args).not.toContain('--dangerously-skip-permissions');
    expect(run.args.slice(run.args.indexOf('--disallowedTools') + 1, run.args.indexOf('--allowedTools'))).toEqual(RESEARCH_DENIED_TOOLS);
    expect(run.args.slice(run.args.indexOf('--allowedTools') + 1)).toEqual(RESEARCH_TOOLS);
    expect(run.prompt).toContain(`harness board question research ${q.id} --from research.json --author research-${q.id}`);
    expect(f.launcher.launched[0].env.HARNESS_TICKET).toBeUndefined();
    expect((await f.service.stores.state.read()).research[q.id]).toMatchObject({ count: 1, autoAt: null });
    const status = await f.service.status(f.boardId);
    expect(status).toMatchObject({ active: 0, research: { auto: false, wip: 1, active: 1, maxPerDay: 10, today: 0 } });
    expect(status.runs).toEqual([expect.objectContaining({ kind: 'research', questionId: q.id })]);
    expect((await f.service.researchStatus(q.id)).run).toMatchObject({ questionId: q.id });
  });

  it('lehnt ab: laufende Recherche, nicht offene Frage, Pause, kein eigenes Board mit Repo im Projekt', async () => {
    f = await DispatchFixture.create();
    const q = await f.services.questions.create('ARC', { title: 'Offen' }, HUMAN);
    await f.service.startResearch(q.id);
    await expect(f.service.startResearch(q.id)).rejects.toMatchObject({ code: 'research-running' });
    const done = await f.services.questions.create('ARC', { title: 'Erledigt' }, HUMAN);
    await f.services.questions.discard(done.id, 'egal', HUMAN, {});
    await expect(f.service.startResearch(done.id)).rejects.toMatchObject({ code: 'question-not-open' });
    await expect(f.service.startResearch('ARC-F0099')).rejects.toMatchObject({ code: 'question-not-found' });
    const other = await f.services.questions.create('ARC', { title: 'Andere' }, HUMAN);
    await f.service.pause(30);
    await expect(f.service.startResearch(other.id)).rejects.toMatchObject({ code: 'dispatch-paused' });
    await f.service.resume();
    await f.services.projects.create('OTH', 'Anderes', f.backend.actor());
    const foreign = await f.services.questions.create('OTH', { title: 'Ohne Board' }, HUMAN);
    await expect(f.service.startResearch(foreign.id)).rejects.toMatchObject({
      code: 'research-no-repo', message: expect.stringContaining(`Für Frage ${foreign.id} ist kein eigenes Board mit Repo-Pfad bekannt`),
    });
  });

  it('nimmt das eigene Board eines verknüpften Tickets, auch wenn das aktive Board ein anderes ist', async () => {
    f = await DispatchFixture.create();
    const ticket = await f.ready('Rabatte');
    const second = await f.services.boards.create('ARC', 'Zweites', f.backend.actor());
    await f.services.boards.setRepoPath(second.id, f.repo.path);
    await new BoardConfigStore(f.home).write({ ...MEMORY_CONFIG, activeBoard: second.id });
    const q = await f.services.questions.create('ARC', { title: 'Zum Ticket', ticketIds: [ticket] }, HUMAN);
    expect((await f.service.startResearch(q.id)).questionId).toBe(q.id);
    expect((await f.run(q.id)).boardId).toBe(f.boardId);
  });

  it('ohne verknüpftes Ticket: aktives Board des Projekts, sonst das eigene Board des Projekts mit Repo und kleinster ID', async () => {
    f = await DispatchFixture.create();
    const second = await f.services.boards.create('ARC', 'Zweites', f.backend.actor());
    await f.services.boards.setRepoPath(second.id, f.repo.path);
    const noRepo = await f.services.boards.create('ARC', 'Ohne Repo', f.backend.actor());
    await new BoardConfigStore(f.home).write({ ...MEMORY_CONFIG, activeBoard: noRepo.id });
    const first = await f.services.questions.create('ARC', { title: 'Eins' }, HUMAN);
    await f.service.startResearch(first.id);
    expect((await f.run(first.id)).boardId).toBe(Math.min(f.boardId, second.id));
    await new BoardConfigStore(f.home).write({ ...MEMORY_CONFIG, activeBoard: second.id });
    const other = await f.services.questions.create('ARC', { title: 'Zwei' }, HUMAN);
    const runner = new ResearchRunner(await contextOf(f));
    expect((await runner.locate(await f.services.questions.get(other.id))).id).toBe(second.id);
  });
});

// DispatchContext wie im Tick (für Prüfungen der Board-Wahl)
async function contextOf(f: DispatchFixture): Promise<DispatchContext> {
  const cfg = (await new DispatchConfigStore(f.home).read()).config;
  const board = await new BoardConfigStore(f.home).require();
  return new DispatchContext(cfg, { userId: board.userId, activeBoard: board.activeBoard }, f.backend, f.service.stores, f.launcher, () => f.now, () => undefined);
}

describe('DispatchService – Recherche auswerten', () => {
  let f: DispatchFixture;
  afterEach(() => f?.dispose());

  it('neue Recherche des Agents: ok, Worktree entfernt, kein Kommentar', async () => {
    f = await DispatchFixture.create({ enabled: false });
    const q = await f.services.questions.create('ARC', { title: 'Frage' }, HUMAN);
    await f.service.startResearch(q.id);
    const run = await f.run(q.id);
    await f.services.questions.addResearch(q.id, RESEARCH, { ...HUMAN, name: `research-${q.id}`, kind: 'session' });
    await f.end(q.id);
    const report = await f.service.tick();
    expect(report.evaluated).toEqual([{ ticketId: q.id, outcome: 'researched' }]);
    expect(existsSync(run.cwd)).toBe(false);
    expect(await f.service.stores.runs.list()).toEqual([]);
    expect((await f.services.questions.get(q.id)).comments).toEqual([]);
  });

  it('eine Recherche eines anderen Autors während des Laufs zählt nicht als Ergebnis', async () => {
    f = await DispatchFixture.create({ enabled: false });
    const q = await f.services.questions.create('ARC', { title: 'Frage' }, HUMAN);
    await f.service.startResearch(q.id);
    await f.services.questions.addResearch(q.id, RESEARCH, HUMAN);
    await f.end(q.id);
    expect((await f.service.tick()).evaluated).toEqual([{ ticketId: q.id, outcome: 'empty' }]);
    expect((await f.services.questions.get(q.id)).comments[0].body).toMatch(/^\[dispatch\] Recherche ohne Ergebnis/);
  });

  it('ohne neue Recherche: Kommentar mit Grund und Ausgabe an der Frage, kein Neuversuch', async () => {
    f = await DispatchFixture.create({ enabled: false });
    await autoOn(f);
    const q = await f.services.questions.create('ARC', { title: 'Frage' }, HUMAN);
    await f.service.startResearch(q.id);
    const run = await f.run(q.id);
    const denied = JSON.stringify({ type: 'result', subtype: 'success', is_error: false, result: 'nichts gefunden', permission_denials: [{ tool_name: 'Edit' }] });
    await f.end(q.id, denied);
    const report = await f.service.tick();
    expect(report.evaluated).toEqual([{ ticketId: q.id, outcome: 'empty' }]);
    expect(report.research).toEqual([]);
    expect(existsSync(run.cwd)).toBe(false);
    const comments = (await f.services.questions.get(q.id)).comments;
    expect(comments).toHaveLength(1);
    expect(comments[0]).toMatchObject({ authorName: `research-${q.id}`, authorKind: 'session' });
    expect(comments[0].body).toMatch(/^\[dispatch\] Recherche ohne Ergebnis: Lauf endete, ohne eine Recherche an der Frage zu speichern; abgelehnte Werkzeugaufrufe: Edit/);
    expect(comments[0].body).toContain('nichts gefunden');
    expect(comments[0].body).toContain(`Protokoll: ${run.id}.log`);
    expect((await f.service.tick()).research).toEqual([]);
  });

  it('Nutzungslimit: Pause, Kommentar, Worktree entfernt', async () => {
    f = await DispatchFixture.create();
    const q = await f.services.questions.create('ARC', { title: 'Frage' }, HUMAN);
    await f.service.startResearch(q.id);
    const run = await f.run(q.id);
    await f.end(q.id, 'Claude usage limit reached', 1);
    const report = await f.service.tick();
    expect(report.evaluated).toEqual([{ ticketId: q.id, outcome: 'limit' }]);
    expect(report.idle).toMatch(/^Pausiert bis/);
    expect(existsSync(run.cwd)).toBe(false);
    expect((await f.services.questions.get(q.id)).comments[0].body).toMatch(/^\[dispatch\] Recherche ohne Ergebnis: Nutzungslimit erreicht/);
    expect(f.notices.at(-1)?.kind).toBe('paused');
    await expect(f.service.startResearch(q.id)).rejects.toMatchObject({ code: 'dispatch-paused' });
  });

  it('wiederholte Auswertung (Verbindungsfehler nach dem Kommentar): kein zweiter Kommentar, keine zweite Pause', async () => {
    f = await DispatchFixture.create({ enabled: false });
    const q = await f.services.questions.create('ARC', { title: 'Frage' }, HUMAN);
    await f.service.startResearch(q.id);
    const agent = { ...HUMAN, name: `research-${q.id}`, kind: 'session' as const };
    await f.services.questions.comment(q.id, '[dispatch] Recherche ohne Ergebnis: Nutzungslimit erreicht. …', agent);
    await f.end(q.id, 'Claude usage limit reached', 1);
    expect((await f.service.tick()).evaluated).toEqual([{ ticketId: q.id, outcome: 'limit' }]);
    expect((await f.services.questions.get(q.id)).comments).toHaveLength(1);
    expect((await f.service.stores.state.read()).pausedUntil).toBeNull();
    expect(f.notices).toEqual([]);

    const other = await f.services.questions.create('ARC', { title: 'Zweite' }, HUMAN);
    await f.service.startResearch(other.id);
    await f.services.questions.comment(other.id, '[dispatch] Recherche ohne Ergebnis: schon da.', { ...agent, name: `research-${other.id}` });
    await f.end(other.id);
    expect((await f.service.tick()).evaluated).toEqual([{ ticketId: other.id, outcome: 'empty' }]);
    expect((await f.services.questions.get(other.id)).comments).toHaveLength(1);
  });

  it('verschwundener Worker: Kommentar mit Grund, Worktree entfernt', async () => {
    f = await DispatchFixture.create({ enabled: false });
    const q = await f.services.questions.create('ARC', { title: 'Frage' }, HUMAN);
    await f.service.startResearch(q.id);
    const run = await f.run(q.id);
    f.alive = false;
    expect((await f.service.tick()).evaluated).toEqual([{ ticketId: q.id, outcome: 'empty' }]);
    expect(existsSync(run.cwd)).toBe(false);
    expect((await f.services.questions.get(q.id)).comments[0].body).toContain('Worker-Prozess endete ohne Ergebnis');
  });

  it('entfernt verwaiste Recherche-Worktrees ohne Lauf bei jedem Tick, laufende bleiben', async () => {
    f = await DispatchFixture.create();
    const q = await f.services.questions.create('ARC', { title: 'Läuft' }, HUMAN);
    await f.service.startResearch(q.id);
    const running = await f.run(q.id);
    const orphan = await new ResearchWorktree().create(running.repoRoot!, 'ARC-F0042');
    await f.service.tick();
    expect(existsSync(orphan.path)).toBe(false);
    expect(existsSync(running.cwd)).toBe(true);
  });
});

describe('DispatchService – Recherche-Automatik', () => {
  let f: DispatchFixture;
  afterEach(() => {
    vi.restoreAllMocks();
    f?.dispose();
  });

  it('aus: nichts; an: nur Fragen ab dem Einschalten, auch bei ausgeschaltetem Dispatcher, älteste zuerst, bis research.wip, je Frage einmal', async () => {
    f = await DispatchFixture.create({ enabled: false });
    const old = await f.services.questions.create('ARC', { title: 'Vor dem Einschalten' }, HUMAN);
    expect((await f.service.tick())).toMatchObject({ idle: 'Dispatcher ist ausgeschaltet', research: [] });
    expect((await f.service.stores.state.read()).researchAuto.since).toBeNull();

    await autoOn(f, { wip: 1 });
    expect((await f.service.stores.state.read()).researchAuto.since).toBe(f.now.toISOString());
    const first = await f.services.questions.create('ARC', { title: 'Erste' }, HUMAN);
    await tick();
    const second = await f.services.questions.create('ARC', { title: 'Zweite' }, HUMAN);
    const researched = await f.services.questions.create('ARC', { title: 'Schon recherchiert' }, HUMAN);
    await f.services.questions.addResearch(researched.id, RESEARCH, HUMAN);
    const report = await f.service.tick();
    expect(report.research).toEqual([first.id]);
    expect(report.started).toEqual([]);
    expect((await f.service.tick()).research).toEqual([]);
    expect((await f.service.stores.state.read()).research[first.id].autoAt).not.toBeNull();

    await f.end(first.id);
    const next = await f.service.tick();
    expect(next.evaluated).toEqual([{ ticketId: first.id, outcome: 'empty' }]);
    expect(next.research).toEqual([second.id]);
    await f.end(second.id);
    expect((await f.service.tick()).research).toEqual([]);
    expect((await f.service.stores.state.read()).research[old.id]).toBeUndefined();
  });

  it('aus- und wieder einschalten: neuer Zeitpunkt, Fragen aus der Zwischenzeit bleiben außen vor', async () => {
    f = await DispatchFixture.create({ enabled: false });
    await autoOn(f);
    const before = (await f.service.stores.state.read()).researchAuto.since;
    await configure(f, { research: { auto: false } });
    await f.service.tick();
    expect((await f.service.stores.state.read()).researchAuto.since).toBeNull();
    const meanwhile = await f.services.questions.create('ARC', { title: 'Zwischendurch' }, HUMAN);
    await autoOn(f);
    const since = (await f.service.stores.state.read()).researchAuto.since;
    expect(since).not.toBe(before);
    expect(Date.parse(since!)).toBeGreaterThan(Date.parse(meanwhile.createdAt));
    const fresh = await f.services.questions.create('ARC', { title: 'Neu' }, HUMAN);
    expect((await f.service.tick()).research).toEqual([fresh.id]);
  });

  it('Tageslimit research.maxPerDay, gezählt im Zustand, am nächsten Tag weiter', async () => {
    f = await DispatchFixture.create({ enabled: false });
    await autoOn(f, { wip: 5, maxPerDay: 2 });
    const ids: string[] = [];
    for (const title of ['Eins', 'Zwei', 'Drei']) {
      ids.push((await f.services.questions.create('ARC', { title }, HUMAN)).id);
      await tick();
    }
    expect((await f.service.tick()).research).toEqual(ids.slice(0, 2));
    expect((await f.service.tick()).research).toEqual([]);
    expect((await f.service.stores.state.read()).researchAuto).toMatchObject({ day: ResearchRunner.day(f.now), count: 2 });
    expect((await f.service.status(f.boardId)).research).toMatchObject({ today: 2, maxPerDay: 2 });
    f.advance(24 * 60);
    expect((await f.service.tick()).research).toEqual([ids[2]]);
    expect((await f.service.stores.state.read()).researchAuto.count).toBe(1);
  });

  it('nur Fragen, die mich betreffen: eigene oder mit Tickets auf eigenen Boards; fremde nur von Hand', async () => {
    f = await DispatchFixture.create({ enabled: false });
    await f.backend.db.transaction((u) => u.users.upsert(OTHER_USER));
    const ticket = await f.ready('Rabatte');
    await autoOn(f, { wip: 5 });
    const foreign = await f.services.questions.create('ARC', { title: 'Fremd' }, OTHER);
    const linked = await f.services.questions.create('ARC', { title: 'Fremd, zu meinem Ticket', ticketIds: [ticket] }, OTHER);
    const mine = await f.services.questions.create('ARC', { title: 'Meine (vom Agent)' }, { ...HUMAN, name: 'junior-ARC-0001', kind: 'session' });
    expect((await f.service.tick()).research.sort()).toEqual([linked.id, mine.id].sort());
    expect((await f.service.tick()).research).toEqual([]);
    expect((await f.service.startResearch(foreign.id)).questionId).toBe(foreign.id);
  });

  it('respektiert Pause und Zeitfenster und zählt Recherchen nicht gegen das WIP der Tickets', async () => {
    f = await DispatchFixture.create({ research: { auto: true, wip: 2 } });
    await autoOn(f, { wip: 2 });
    const ticket = await f.ready('Ticket');
    const q1 = await f.services.questions.create('ARC', { title: 'Eins' }, HUMAN);
    const q2 = await f.services.questions.create('ARC', { title: 'Zwei' }, HUMAN);
    await f.service.pause(10);
    expect((await f.service.tick()).research).toEqual([]);
    await f.service.resume();
    const report = await f.service.tick();
    expect(report.started).toEqual([ticket]);
    expect(report.research.sort()).toEqual([q1.id, q2.id].sort());
    const status = await f.service.status(f.boardId);
    expect(status).toMatchObject({ active: 1, wip: 1, research: { active: 2, wip: 2 } });
  });

  it('Fehler der Frage (kein Repo) merken die Frage als erledigt; die übrigen werden weiter bedient', async () => {
    f = await DispatchFixture.create({ enabled: false });
    await autoOn(f, { wip: 5 });
    const broken = await f.services.questions.create('ARC', { title: 'Kaputt' }, HUMAN);
    await tick();
    const fine = await f.services.questions.create('ARC', { title: 'Gut' }, HUMAN);
    const original = ResearchRunner.prototype.start;
    vi.spyOn(ResearchRunner.prototype, 'start').mockImplementation(function (this: ResearchRunner, id: string, mode: 'manual' | 'auto') {
      if (id === broken.id) return Promise.reject(new HarnessError('research-no-repo', `Für Frage ${id} ist kein eigenes Board mit Repo-Pfad bekannt`));
      return original.call(this, id, mode);
    });
    const first = await f.service.tick();
    expect(first.research).toEqual([fine.id]);
    expect(first.errors).toEqual([expect.stringContaining(`${broken.id}: Für Frage ${broken.id} ist kein eigenes Board`)]);
    expect((await f.service.stores.state.read()).research[broken.id].autoAt).not.toBeNull();
    vi.restoreAllMocks();
    expect((await f.service.tick())).toMatchObject({ research: [], errors: [] });
  });

  it('Umgebungsfehler: Abbruch ohne Merken, einmal je Fehlertext gemeldet, Neuversuch frühestens nach 30 min', async () => {
    f = await DispatchFixture.create({ enabled: false });
    await autoOn(f, { wip: 5 });
    const q1 = await f.services.questions.create('ARC', { title: 'Eins' }, HUMAN);
    await tick();
    const q2 = await f.services.questions.create('ARC', { title: 'Zwei' }, HUMAN);
    f.launcher.fail = new Error('Git-Index gesperrt');
    const first = await f.service.tick();
    expect(first.research).toEqual([]);
    expect(first.errors).toEqual([expect.stringMatching(/^Recherche-Automatik ausgesetzt bis \d\d:\d\d \(bei .*\): Git-Index gesperrt$/)]);
    const state = await f.service.stores.state.read();
    expect(state.research).toEqual({});
    expect(state.researchAuto).toMatchObject({ lastError: 'Git-Index gesperrt', retryAt: expect.any(String) });
    expect((await f.service.status(f.boardId)).research).toMatchObject({ error: 'Git-Index gesperrt', retryAt: state.researchAuto.retryAt });
    expect(f.repo.git('worktree', 'list')).not.toContain('research-');

    f.advance(10);
    expect((await f.service.tick())).toMatchObject({ research: [], errors: [] });
    f.advance(25);
    expect((await f.service.tick())).toMatchObject({ research: [], errors: [] });
    expect(Date.parse((await f.service.stores.state.read()).researchAuto.retryAt!)).toBeGreaterThan(f.now.getTime());

    f.launcher.fail = null;
    f.advance(31);
    expect((await f.service.tick()).research).toEqual([q1.id, q2.id]);
    expect((await f.service.stores.state.read()).researchAuto).toMatchObject({ lastError: null, retryAt: null });
  });

  it('Repo-Pfad ohne Git-Repo gilt als Umgebungsfehler, nicht als Fehler der Frage', async () => {
    f = await DispatchFixture.create({ enabled: false });
    await autoOn(f);
    const q = await f.services.questions.create('ARC', { title: 'Eins' }, HUMAN);
    await f.services.boards.setRepoPath(f.boardId, f.dir);
    const report = await f.service.tick();
    expect(report.errors).toEqual([expect.stringContaining(`Recherche-Automatik ausgesetzt bis`)]);
    expect((await f.service.stores.state.read()).research[q.id]).toBeUndefined();
  });
});

describe('DispatchService – Recherche mit gesperrtem Lauf', () => {
  let f: DispatchFixture;
  afterEach(() => f?.dispose());

  it('ein Startfehler des Workers lässt keinen Lauf und keinen Worktree zurück', async () => {
    f = await DispatchFixture.create();
    const q = await f.services.questions.create('ARC', { title: 'Frage' }, HUMAN);
    f.launcher.fail = new Error('claude fehlt');
    await expect(f.service.startResearch(q.id)).rejects.toThrow('claude fehlt');
    expect(await f.service.stores.runs.list()).toEqual([]);
    expect(f.repo.git('worktree', 'list')).not.toContain('research-');
  });
});
