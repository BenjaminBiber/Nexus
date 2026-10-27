import { existsSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { DispatchFixture } from '../../helpers/DispatchFixture.js';

describe('DispatchService – ein Worktree je Ticket', () => {
  let f: DispatchFixture;
  afterEach(() => f?.dispose());

  // Agent committet eine Datei im Worktree und setzt das Ticket auf Blockiert mit Grund „Human Interaction: …“
  async function humanInteraction(id: string, file: string): Promise<void> {
    const ticket = await f.services.tickets.get(id);
    writeFileSync(join(ticket.worktree!, file), id);
    f.repo.git('-C', ticket.worktree!, 'add', file);
    f.repo.git('-C', ticket.worktree!, 'commit', '-q', '-m', `${id}: ${file}`);
    await f.services.tickets.update(id, { status: 'blocked', blockedReason: 'Human Interaction: Oberfläche ansehen' }, f.agent(id));
  }

  it('drei bereite, unabhängige Tickets derselben Spec starten bei WIP 3 gleichzeitig, jedes im eigenen Worktree', async () => {
    f = await DispatchFixture.create({ wip: 3 });
    const ids = [
      await f.ready('Erstes', { spec: '001-x', specTask: 'T001', touches: ['a'] }),
      await f.ready('Zweites', { spec: '001-x', specTask: 'T002', touches: ['b'] }),
      await f.ready('Drittes', { spec: '001-x', specTask: 'T003', touches: ['c'] }),
    ];
    const report = await f.service.tick();
    expect(report.started).toEqual(ids);
    expect(report.errors).toEqual([]);
    const tickets = await Promise.all(ids.map((id) => f.services.tickets.get(id)));
    expect(new Set(tickets.map((t) => t.branch)).size).toBe(3);
    expect(new Set(tickets.map((t) => t.worktree)).size).toBe(3);
    for (const t of tickets) {
      expect(t.branch).toBe(`ticket/${t.id}-${t.title.toLowerCase()}`);
      expect(existsSync(t.worktree!)).toBe(true);
    }
  });

  it('Voraussetzung erledigt, aber noch nicht gemergt: das Folgeticket wartet; nach dem Merge startet es mit ihrem Code', async () => {
    f = await DispatchFixture.create();
    const base = await f.ready('Basis', { labels: [] });
    const next = await f.ready('Folge', { dependsOn: [base] });
    // Basis setzt ein Mensch von Hand um (kein Merge-Auftrag des Dispatchers)
    const manager = await f.manager();
    const started = await manager.start(base, {}, f.repo.path, f.backend.actor());
    writeFileSync(join(started.path, 'basis.txt'), 'basis');
    f.repo.git('-C', started.path, 'add', 'basis.txt');
    f.repo.git('-C', started.path, 'commit', '-q', '-m', 'basis');
    await manager.finish(base, {}, f.repo.path, f.backend.actor());
    expect(await f.services.tickets.get(base)).toMatchObject({ status: 'done', branch: started.branch });

    expect((await f.service.tick()).started).toEqual([]);
    expect((await f.service.status(f.boardId)).tickets).toEqual([{ id: next, title: 'Folge', reason: `wartet auf Merge von ${base}` }]);

    // „Mergen“ im Panel: der Branch ist danach weg, im selben Tick startet das Folgeticket
    const report = await f.service.retryMerge(base);
    expect(report.merged).toEqual([base]);
    expect((await f.services.tickets.get(base)).branch).toBeNull();
    expect(report.started).toEqual([next]);
    const branch = (await f.services.tickets.get(next)).branch!;
    expect(f.repo.git('show', `${branch}:basis.txt`)).toBe('basis');
  });

  it('ein fertiges Ticket wird gemergt, obwohl andere Tickets derselben Spec noch nicht gestartet sind', async () => {
    f = await DispatchFixture.create();
    const first = await f.ready('Erstes', { spec: '001-x', specTask: 'T001', touches: ['a'] });
    const second = await f.ready('Zweites', { spec: '001-x', specTask: 'T002', touches: ['b'] });
    expect((await f.service.tick()).started).toEqual([first]);
    await f.finishAsAgent(first, 'a.txt', 'a');
    await f.end(first);
    const report = await f.service.tick();
    expect(report.merged).toEqual([first]);
    expect(existsSync(join(f.repo.path, 'a.txt'))).toBe(true);
    // Das zweite Ticket startet danach auf eigenem Branch (vom gemergten Stand)
    expect(report.started).toEqual([second]);
    const t2 = await f.services.tickets.get(second);
    expect(t2.branch).toBe(`ticket/${second}-zweites`);
    expect(f.repo.git('show', `${t2.branch}:a.txt`)).toBe('a');
  });

  it('Human Interaction wird erst nach „Abnehmen“ gemergt und hält andere Tickets nicht auf', async () => {
    f = await DispatchFixture.create({ wip: 2 });
    const look = await f.ready('Ansehen', { spec: '001-x', specTask: 'T001', touches: ['a'] });
    const other = await f.ready('Anderes', { spec: '001-x', specTask: 'T002', touches: ['b'] });
    expect((await f.service.tick()).started).toEqual([look, other]);
    await humanInteraction(look, 'ansehen.txt');
    await f.end(look);
    await f.finishAsAgent(other, 'b.txt', 'b');
    await f.end(other);
    const report = await f.service.tick();
    expect(report.evaluated).toEqual(expect.arrayContaining([{ ticketId: look, outcome: 'blocked' }, { ticketId: other, outcome: 'done' }]));
    expect(report.merged).toEqual([other]);
    expect(existsSync(join(f.repo.path, 'ansehen.txt'))).toBe(false);
    expect(await f.services.tickets.get(look)).toMatchObject({ status: 'blocked', branch: `ticket/${look}-ansehen` });

    const accepted = await f.service.accept(look, 'Florian');
    expect(accepted.result).toMatchObject({ outcome: 'merged', text: expect.stringContaining(`${look} gemergt in`) });
    expect(await f.services.tickets.get(look)).toMatchObject({ status: 'done', blockedReason: null, branch: null, worktree: null });
    expect(existsSync(join(f.repo.path, 'ansehen.txt'))).toBe(true);
    const history = (await f.services.tickets.get(look)).history;
    expect(history.some((h) => h.field === 'status' && h.to === 'done' && h.note === 'abgenommen' && h.byName === 'Florian')).toBe(true);
  });

  it('Einrichtung nur im neu angelegten Worktree; gelungen wird beim Fortsetzen nicht erneut eingerichtet', async () => {
    const steps = [{ cwd: 'app', run: 'npm ci --prefer-offline --no-audit' }];
    f = await DispatchFixture.create({ worktreeSetup: steps, worktreeSetupTimeoutMinutes: 7 });
    const id = await f.ready('Einrichten');
    await f.service.tick();
    const first = await f.run(id);
    expect(first).toMatchObject({ setup: steps, setupTimeoutMinutes: 7 });
    expect(first.prompt).toContain('laut Einrichtung des Dispatchers installiert');
    // Einrichtung gelungen, Claude bricht ab → Fortsetzung ohne Einrichtung
    await f.service.stores.runs.finish(first.id, { code: 1, timedOut: false, output: 'x', setupOk: true, endedAt: new Date().toISOString() });
    f.advance(1);
    // Abbruch auswerten und im selben Tick wieder starten
    expect((await f.service.tick()).started).toEqual([id]);
    expect(await (await f.manager()).setupPending(first.branch)).toBe(false);
    const second = await f.run(id);
    expect(second.id).not.toBe(first.id);
    expect(second.setup).toBeUndefined();
  });

  it('Einrichtung gescheitert: Blockiert mit Grund, Ausgabe als Kommentar, kein Versuch; beim nächsten Start erneut eingerichtet', async () => {
    const steps = [{ cwd: '.', run: 'npm ci' }];
    f = await DispatchFixture.create({ worktreeSetup: steps });
    const id = await f.ready('Kaputt');
    await f.service.tick();
    const first = await f.run(id);
    await f.service.stores.runs.finish(first.id, {
      code: 1, timedOut: false, output: 'npm ERR! kaputt', endedAt: new Date().toISOString(),
      setupFailed: { cwd: '.', run: 'npm ci', code: 1, timedOut: false, output: 'npm ERR! kaputt' },
    });
    const report = await f.service.tick();
    expect(report.evaluated).toEqual([{ ticketId: id, outcome: 'setup-failed' }]);
    const ticket = await f.services.tickets.get(id);
    expect(ticket).toMatchObject({ status: 'blocked', blockedReason: 'Einrichtung fehlgeschlagen: .$ npm ci (Exit 1)', assignee: null, branch: first.branch });
    const comment = (await f.comments(id)).at(-1)!;
    expect(comment).toMatch(/^\[dispatch\] Einrichtung fehlgeschlagen: \.\$ npm ci \(Exit 1\)/);
    expect(comment).toContain('npm ERR! kaputt');
    expect((await f.comments(id)).some((c) => c.startsWith('[dispatch] Abbruch'))).toBe(false);
    expect(f.notices.at(-1)).toMatchObject({ kind: 'blocked', ticketId: id });
    // Nach dem Beheben wieder Geplant: Fortsetzung im selben Worktree mit erneuter Einrichtung
    await f.services.tickets.update(id, { status: 'planned' }, f.backend.actor(), { force: true });
    f.advance(1);
    expect((await f.service.tick()).started).toEqual([id]);
    const second = await f.run(id);
    expect(second).toMatchObject({ branch: first.branch, cwd: first.cwd, setup: steps });
  });

  it('„Abnehmen“ lehnt Tickets ohne Human Interaction ab', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Normal');
    await expect(f.service.accept(id)).rejects.toMatchObject({ code: 'accept-not-possible' });
  });

  it('Ticket mit Branch auf den Worktree eines anderen Tickets: wird nicht wiederhergestellt, sondern startet auf eigenem Branch', async () => {
    f = await DispatchFixture.create();
    const host = await f.ready('Fremd', { labels: [] });
    const id = await f.ready('Verirrt');
    const manager = await f.manager();
    const foreign = await manager.start(host, {}, f.repo.path, f.backend.actor());
    f.repo.git('worktree', 'remove', '--force', foreign.path);
    await f.services.tickets.update(id, { branch: foreign.branch, worktree: foreign.path }, f.backend.actor());
    const report = await f.service.tick();
    expect(report.started).toEqual([id]);
    expect(existsSync(foreign.path)).toBe(false);
    const ticket = await f.services.tickets.get(id);
    expect(ticket).toMatchObject({ status: 'in_progress', branch: `ticket/${id}-verirrt` });
    expect(ticket.worktree).not.toBe(foreign.path);
    const start = (await f.comments(id)).find((c) => c.startsWith('[dispatch] Start'))!;
    expect(start).toContain(`Hinweis: Der eingetragene Worktree ${foreign.path} (Branch ${foreign.branch}) gehört ${host}`);
  });
});
