import { existsSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { BranchMerger, DispatchConfigStore, HarnessError, TargetBranches } from '@harness/core';
import { MergeService } from '../../../src/dispatch/MergeService.js';
import { DispatchFixture } from '../../helpers/DispatchFixture.js';

describe('DispatchService – Merge', () => {
  let f: DispatchFixture;
  afterEach(() => {
    vi.restoreAllMocks();
    f?.dispose();
  });

  // Ausgecheckter Branch des Hauptordners
  const mainBranch = () => f.repo.git('rev-parse', '--abbrev-ref', 'HEAD');

  // Startet ein bereites Ticket, lässt den Agenten fertig werden und wertet aus
  async function doneByAgent(title: string, file = 'feature.txt', content = 'neu'): Promise<string> {
    const id = await f.ready(title);
    await f.service.tick();
    await f.finishAsAgent(id, file, content);
    await f.end(id);
    return id;
  }

  // Hauptordner bekommt einen eigenen Stand der Datei (Konflikt mit dem Ticket)
  function commitOnMain(file = 'feature.txt', content = 'haupt'): void {
    writeFileSync(join(f.repo.path, file), content);
    f.repo.git('add', file);
    f.repo.git('commit', '-q', '-m', `haupt ${file}`);
  }

  // Ticket bis zur laufenden Merge-Session bringen (Konflikt in feature.txt)
  async function inMergeSession(title: string): Promise<string> {
    const id = await f.ready(title);
    await f.service.tick();
    commitOnMain();
    await f.finishAsAgent(id, 'feature.txt', 'ticket');
    await f.end(id);
    await f.service.tick();
    return id;
  }

  // Zwei Tickets derselben Spec in einem gemeinsamen Worktree (nacheinander, nie zwei Worker zugleich), beide erledigt
  // und ausgewertet (Konflikt optional in a.txt)
  async function sharedDone(conflict = false): Promise<[string, string]> {
    const first = await f.ready('Erstes', { spec: '001-x', specTask: 'T001', touches: ['a'] });
    const second = await f.ready('Zweites', { spec: '001-x', specTask: 'T002', touches: ['b'] });
    await f.service.tick();
    if (conflict) commitOnMain('a.txt', 'haupt');
    await f.finishAsAgent(first, 'a.txt', 'a');
    await f.end(first);
    // Erst jetzt ist der Worktree frei: das zweite Ticket tritt bei
    expect((await f.service.tick()).started).toEqual([second]);
    await f.finishAsAgent(second, 'b.txt', 'b');
    await f.end(second);
    return [first, second];
  }

  // Merge-Sessions im RunStore
  async function mergeRuns() {
    return (await f.service.stores.runs.list()).filter((r) => r.kind === 'merge');
  }

  it('mergt nach Erledigt in den ausgecheckten Branch und räumt Worktree und Branch auf', async () => {
    f = await DispatchFixture.create();
    const id = await doneByAgent('Login');
    const worktree = (await f.services.tickets.get(id)).worktree!;
    const report = await f.service.tick();
    expect(report.merged).toEqual([id]);
    expect(readFileSync(join(f.repo.path, 'feature.txt'), 'utf8')).toBe('neu');
    expect(f.repo.git('log', '-1', '--format=%P').split(' ')).toHaveLength(2);
    expect(f.repo.git('log', '-1', '--format=%s')).toBe(`Merge ${id} „Login“ (ticket/${id}-login)`);
    expect(existsSync(worktree)).toBe(false);
    expect(f.repo.git('branch', '--list', `ticket/${id}-login`)).toBe('');
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'done', branch: null, worktree: null });
    expect((await f.comments(id)).at(-1)).toMatch(new RegExp(`^\\[dispatch\\] Gemergt in ${mainBranch()} \\(`));
    expect(f.notices.at(-1)).toMatchObject({ kind: 'merged', ticketId: id });
    expect((await f.service.stores.state.read()).merges).toEqual({});
  });

  it('lokale Änderungen in anderen Dateien halten den Merge nicht an und bleiben stehen', async () => {
    f = await DispatchFixture.create();
    const id = await doneByAgent('Bericht');
    writeFileSync(join(f.repo.path, 'lokal.txt'), 'x');
    expect((await f.service.tick()).merged).toEqual([id]);
    expect(readFileSync(join(f.repo.path, 'feature.txt'), 'utf8')).toBe('neu');
    expect(readFileSync(join(f.repo.path, 'lokal.txt'), 'utf8')).toBe('x');
    expect(f.repo.git('status', '--porcelain')).toBe('?? lokal.txt');
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'done', branch: null });
  });

  it('überschneidende lokale Änderungen: wartet (einmal kommentiert) und mergt danach von selbst', async () => {
    f = await DispatchFixture.create();
    commitOnMain('cfg.yaml', 'alt');
    const id = await doneByAgent('Konfig', 'cfg.yaml', 'ticket');
    writeFileSync(join(f.repo.path, 'cfg.yaml'), 'lokal');
    const notices = f.notices.length;
    expect((await f.service.tick()).merged).toEqual([]);
    expect(await f.service.tick()).toMatchObject({ merged: [], errors: [] });
    const ticket = await f.services.tickets.get(id);
    expect(ticket).toMatchObject({ status: 'done', blockedReason: null });
    expect(existsSync(ticket.worktree!)).toBe(true);
    const waits = (await f.comments(id)).filter((c) => c.startsWith('[dispatch] Merge wartet auf den Hauptordner'));
    expect(waits).toEqual(['[dispatch] Merge wartet auf den Hauptordner: lokale Änderungen in cfg.yaml, die der Branch ebenfalls ändert – gemergt wird automatisch, sobald das erledigt ist.']);
    expect(f.notices.length).toBe(notices);
    expect((await f.service.stores.state.read()).merges[id]).toMatchObject({ phase: 'waiting' });
    expect(readFileSync(join(f.repo.path, 'cfg.yaml'), 'utf8')).toBe('lokal');
    f.repo.git('checkout', '--', 'cfg.yaml');
    expect((await f.service.tick()).merged).toEqual([id]);
    expect(readFileSync(join(f.repo.path, 'cfg.yaml'), 'utf8')).toBe('ticket');
  });

  it('vorgemerkte Änderungen (git add) lassen den Merge warten', async () => {
    f = await DispatchFixture.create();
    const id = await doneByAgent('Index');
    writeFileSync(join(f.repo.path, 'lokal.txt'), 'x');
    f.repo.git('add', 'lokal.txt');
    expect((await f.service.tick()).merged).toEqual([]);
    expect((await f.comments(id)).at(-1)).toBe('[dispatch] Merge wartet auf den Hauptordner: vorgemerkte Änderungen (git add) in lokal.txt – gemergt wird automatisch, sobald das erledigt ist.');
    const report = await f.service.retryMerge(id);
    expect(report.result).toEqual({ outcome: 'waiting', text: `${id}: Merge wartet auf den Hauptordner: vorgemerkte Änderungen (git add) in lokal.txt – gemergt wird automatisch, sobald das erledigt ist.` });
    f.repo.git('commit', '-q', '-m', 'lokal');
    expect((await f.service.tick()).merged).toEqual([id]);
  });

  it('laufende Git-Aktion im Hauptordner lässt den Merge warten statt zu blockieren', async () => {
    f = await DispatchFixture.create();
    const id = await doneByAgent('Rebase');
    const marker = join(f.repo.path, '.git', 'MERGE_HEAD');
    writeFileSync(marker, f.repo.git('rev-parse', 'HEAD'));
    expect((await f.service.tick()).merged).toEqual([]);
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'done' });
    expect((await f.comments(id)).at(-1)).toBe('[dispatch] Merge wartet auf den Hauptordner: Im Hauptordner läuft gerade ein Merge. – gemergt wird automatisch, sobald das erledigt ist.');
    rmSync(marker);
    expect((await f.service.tick()).merged).toEqual([id]);
  });

  it('mergeStrategy squash: ein Commit je Ticket mit Trailer, Worktree und Branch aufgeräumt', async () => {
    f = await DispatchFixture.create({ mergeStrategy: 'squash' });
    const id = await doneByAgent('Login');
    const ticket = await f.services.tickets.get(id);
    const tip = f.repo.git('rev-parse', ticket.branch!);
    const before = f.repo.git('rev-parse', 'HEAD');
    const report = await f.service.tick();
    expect(report.merged).toEqual([id]);
    expect(readFileSync(join(f.repo.path, 'feature.txt'), 'utf8')).toBe('neu');
    expect(f.repo.git('log', '-1', '--format=%P')).toBe(before);
    expect(f.repo.git('log', '-1', '--format=%s')).toBe(`${id} „Login“ (squash von ticket/${id}-login)`);
    expect(f.repo.git('log', '-1', '--format=%(trailers:key=Harness-Squashed,valueonly)')).toBe(`ticket/${id}-login ${tip}`);
    expect(existsSync(ticket.worktree!)).toBe(false);
    expect(f.repo.git('branch', '--list', ticket.branch!)).toBe('');
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'done', branch: null, worktree: null });
    expect((await f.comments(id)).at(-1)).toMatch(new RegExp(`^\\[dispatch\\] Gemergt in ${mainBranch()} \\(.*aufgeräumt`));
  });

  it('mergeStrategy squash: nach der Merge-Session wird der Branch zusammengefasst', async () => {
    f = await DispatchFixture.create({ mergeStrategy: 'squash' });
    const id = await f.ready('Konflikt');
    await f.service.tick();
    commitOnMain();
    await f.finishAsAgent(id, 'feature.txt', 'ticket');
    await f.end(id);
    await f.service.tick();
    const ticket = await f.services.tickets.get(id);
    expect(await f.run(id)).toMatchObject({ kind: 'merge', target: mainBranch() });
    expect(f.repo.git('status', '--porcelain')).toBe('');
    f.repo.git('-C', ticket.worktree!, 'merge', '-q', '-s', 'ours', '--no-edit', mainBranch());
    await f.end(id);
    const before = f.repo.git('rev-parse', 'HEAD');
    expect((await f.service.tick()).merged).toEqual([id]);
    expect(f.repo.git('log', '-1', '--format=%P')).toBe(before);
    expect(f.repo.git('log', '-1', '--format=%s')).toBe(`${id} „Konflikt“ (squash von ${ticket.branch})`);
    expect(readFileSync(join(f.repo.path, 'feature.txt'), 'utf8')).toBe('ticket');
    expect(f.repo.git('branch', '--list', ticket.branch!)).toBe('');
  });

  it('startet bei Konflikt eine Merge-Session und mergt danach', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Konflikt');
    await f.service.tick();
    writeFileSync(join(f.repo.path, 'feature.txt'), 'haupt');
    f.repo.git('add', 'feature.txt');
    f.repo.git('commit', '-q', '-m', 'haupt');
    await f.finishAsAgent(id, 'feature.txt', 'ticket');
    await f.end(id);
    await f.service.tick();
    const session = await f.run(id);
    const ticket = await f.services.tickets.get(id);
    expect(session).toMatchObject({ kind: 'merge', target: mainBranch(), cwd: ticket.worktree, branch: ticket.branch });
    expect(session.args).toContain('Bash(git merge:*)');
    expect(session.prompt).toContain(`git merge ${mainBranch()}`);
    expect((await f.comments(id)).at(-1)).toMatch(/^\[dispatch\] Merge-Konflikt in feature\.txt beim Merge in /);
    expect((await f.service.stores.state.read()).merges[id]).toMatchObject({ phase: 'session', sessionUsed: true });
    expect(f.repo.git('status', '--porcelain')).toBe('');
    // Die Merge-Session löst den Konflikt im Worktree (hier: Ticket-Stand behalten)
    f.repo.git('-C', ticket.worktree!, 'merge', '-q', '-s', 'ours', '--no-edit', mainBranch());
    await f.end(id);
    const report = await f.service.tick();
    expect(report.merged).toEqual([id]);
    expect(readFileSync(join(f.repo.path, 'feature.txt'), 'utf8')).toBe('ticket');
  });

  it('blockiert, wenn die Merge-Session den Konflikt nicht löst', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Ungelöst');
    await f.service.tick();
    writeFileSync(join(f.repo.path, 'feature.txt'), 'haupt');
    f.repo.git('add', 'feature.txt');
    f.repo.git('commit', '-q', '-m', 'haupt');
    await f.finishAsAgent(id, 'feature.txt', 'ticket');
    await f.end(id);
    await f.service.tick();
    await f.end(id);
    await f.service.tick();
    const ticket = await f.services.tickets.get(id);
    expect(ticket.status).toBe('blocked');
    expect(ticket.blockedReason).toMatch(/^Merge: Die Merge-Session konnte den Konflikt nicht lösen/);
  });

  it('blockiert, wenn der eigene Merge nicht zurückgesetzt werden kann (Hauptordner im Merge-Zustand)', async () => {
    f = await DispatchFixture.create();
    const id = await doneByAgent('Abbruch');
    // Ein scheiterndes `git merge --abort` lässt sich im Test nicht zuverlässig erzeugen; BranchMerger wirft dann so
    vi.spyOn(BranchMerger.prototype, 'merge').mockImplementation(() => {
      throw new HarnessError('merge-abort-failed', 'Der Hauptordner steht noch im Merge-Zustand: kaputt. Bitte von Hand aufräumen (git merge --abort).');
    });
    const report = await f.service.tick();
    expect(report.merged).toEqual([]);
    expect(report.errors).toEqual([]);
    const ticket = await f.services.tickets.get(id);
    expect(ticket).toMatchObject({ status: 'blocked' });
    expect(ticket.blockedReason).toBe('Merge: Der Hauptordner steht noch im Merge-Zustand: kaputt. Bitte von Hand aufräumen (git merge --abort).');
    expect((await f.service.stores.state.read()).merges).toEqual({});
  });

  it('wartet mit dem Merge, bis alle Tickets des gemeinsamen Worktrees fertig sind', async () => {
    f = await DispatchFixture.create({ wip: 2, joinWorktrees: 'sameSpec' });
    const first = await f.ready('Erstes', { spec: '001-x', specTask: 'T001', touches: ['a'] });
    const second = await f.ready('Zweites', { spec: '001-x', specTask: 'T002', touches: ['b'] });
    await f.service.tick();
    await f.finishAsAgent(first, 'a.txt', 'a');
    await f.end(first);
    const waiting = await f.service.tick();
    expect(waiting.merged).toEqual([]);
    expect(waiting.started).toEqual([second]);
    expect((await f.services.tickets.get(second)).branch).toBe((await f.services.tickets.get(first)).branch);
    // Zum Zeitpunkt der Auswertung war das zweite Ticket noch nicht beigetreten
    expect((await f.comments(first)).at(-1)).toBe('[dispatch] Merge wartet auf weitere Tickets derselben Spec – der Branch wird gemergt, sobald alle Tickets des Worktrees fertig sind.');
    await f.finishAsAgent(second, 'b.txt', 'b');
    await f.end(second);
    expect((await f.service.tick()).merged.sort()).toEqual([first, second]);
    expect(existsSync(join(f.repo.path, 'a.txt')) && existsSync(join(f.repo.path, 'b.txt'))).toBe(true);
  });

  it('kein Merge, solange ein Ticket des gemeinsamen Worktrees wieder in Arbeit ist', async () => {
    f = await DispatchFixture.create({ wip: 2, joinWorktrees: 'sameSpec' });
    const [first, second] = await sharedDone(true);
    await f.service.tick();
    const [session] = await mergeRuns();
    expect(session.ticketId).toBe(first);
    // Während der Merge-Session geht B zurück auf In Arbeit
    await f.services.tickets.move(second, 'in_progress', f.backend.actor(), { force: true });
    const head = f.repo.git('rev-parse', 'HEAD');
    f.repo.git('-C', (await f.services.tickets.get(first)).worktree!, 'merge', '-q', '-s', 'ours', '--no-edit', mainBranch());
    await f.end(first);
    const report = await f.service.tick();
    expect(report.merged).toEqual([]);
    expect(f.repo.git('rev-parse', 'HEAD')).toBe(head);
    expect((await f.services.tickets.get(first)).status).toBe('done');
    expect((await f.comments(first)).some((c) => c.startsWith('[dispatch] Gemergt'))).toBe(false);
    expect((await f.service.stores.state.read()).merges[first]).toMatchObject({ phase: 'waiting' });
  });

  it('„Erneut mergen“ nur für Tickets, die auf einen Merge warten', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Nichts');
    await expect(f.service.retryMerge(id)).rejects.toMatchObject({ code: 'merge-not-possible' });
  });

  it('gemeinsamer Worktree mit Konflikt: genau eine Merge-Session für alle Tickets des Branches', async () => {
    f = await DispatchFixture.create({ wip: 2, joinWorktrees: 'sameSpec' });
    const [first, second] = await sharedDone(true);
    await f.service.tick();
    const sessions = await mergeRuns();
    expect(sessions).toHaveLength(1);
    const { merges } = await f.service.stores.state.read();
    expect(merges[first]).toMatchObject({ phase: 'session', sessionUsed: true, target: mainBranch() });
    expect(merges[second]).toMatchObject({ phase: 'session', sessionUsed: true, target: mainBranch() });
    // Weiterer Tick, solange die Session läuft: nichts Neues
    await f.service.tick();
    expect(await mergeRuns()).toHaveLength(1);
    // Ungelöst → beide erledigten Tickets des Worktrees blockiert
    await f.service.stores.runs.finish(sessions[0].id, { code: 1, timedOut: false, output: '', endedAt: new Date().toISOString() });
    await f.service.tick();
    for (const id of [first, second]) {
      const t = await f.services.tickets.get(id);
      expect(t.status).toBe('blocked');
      expect(t.blockedReason).toMatch(/^Merge: Die Merge-Session konnte den Konflikt nicht lösen/);
    }
    expect((await f.service.stores.state.read()).merges).toEqual({});
  });

  it('blockiert, wenn der Zielbranch nach der Merge-Session gewechselt wurde', async () => {
    f = await DispatchFixture.create();
    const id = await inMergeSession('Ziel');
    const old = mainBranch();
    const ticket = await f.services.tickets.get(id);
    f.repo.git('-C', ticket.worktree!, 'merge', '-q', '-s', 'ours', '--no-edit', old);
    f.repo.git('checkout', '-q', '-b', 'anderer');
    await f.end(id);
    const report = await f.service.tick();
    expect(report.merged).toEqual([]);
    const blocked = await f.services.tickets.get(id);
    expect(blocked.status).toBe('blocked');
    expect(blocked.blockedReason).toBe(`Merge: Zielbranch gewechselt (Konflikt gelöst gegen ${old}, ausgecheckt ist anderer) – bitte Erneut mergen.`);
    expect(f.repo.git('log', '-1', '--format=%P').split(' ')).toHaveLength(1);
  });

  it('„Erneut mergen“ wird abgelehnt, solange die Merge-Session läuft', async () => {
    f = await DispatchFixture.create();
    const id = await inMergeSession('Läuft');
    await expect(f.service.retryMerge(id)).rejects.toMatchObject({ code: 'merge-not-possible', message: `${id}: Die Merge-Session läuft noch.` });
    expect((await f.service.stores.state.read()).merges[id]).toMatchObject({ phase: 'session', sessionUsed: true });
  });

  it('„Erneut mergen“ setzt einen hängenden Auftrag in Phase „Session“ ohne Merge-Lauf zurück', async () => {
    f = await DispatchFixture.create();
    const id = await inMergeSession('Hängt fest');
    await f.service.stores.runs.remove((await f.run(id)).id);
    const report = await f.service.retryMerge(id);
    expect(report.result).toEqual({ outcome: 'session', text: `${id}: Merge-Konflikt – Merge-Session gestartet.` });
    // Erneuter Versuch: Konflikt, neue Merge-Session
    expect(await mergeRuns()).toHaveLength(1);
    expect((await f.service.stores.state.read()).merges[id]).toMatchObject({ phase: 'session', sessionUsed: true });
    expect((await f.services.tickets.get(id)).status).toBe('done');
  });

  it('„Erneut mergen“ und der folgende Tick laufen unter derselben Sperre wie ein Timer-Tick', async () => {
    f = await DispatchFixture.create();
    const id = await doneByAgent('Sperre');
    f.repo.git('checkout', '-q', '--detach');
    await f.service.tick();
    expect((await f.services.tickets.get(id)).status).toBe('blocked');
    f.repo.git('checkout', '-q', '-');
    const order: string[] = [];
    let release!: () => void;
    const gate = new Promise<void>((r) => { release = r; });
    const retry = MergeService.prototype.retry;
    vi.spyOn(MergeService.prototype, 'retry').mockImplementation(async function (this: MergeService, ...args) {
      order.push('retry');
      await gate;
      await retry.apply(this, args);
      order.push('retry fertig');
    });
    const processAll = MergeService.prototype.processAll;
    vi.spyOn(MergeService.prototype, 'processAll').mockImplementation(async function (this: MergeService, ...args) {
      order.push('merge-tick');
      return processAll.apply(this, args);
    });
    const retried = f.service.retryMerge(id);
    await new Promise((r) => setTimeout(r, 20));
    const timer = f.service.tick();
    await new Promise((r) => setTimeout(r, 300));
    release();
    const report = await retried;
    await timer;
    expect(report.merged).toEqual([id]);
    expect(order.slice(0, 3)).toEqual(['retry', 'retry fertig', 'merge-tick']);
  });

  it('Merge-Session endet, Ticket steht nicht mehr auf Erledigt: Auftrag nur entfernen', async () => {
    f = await DispatchFixture.create();
    const id = await inMergeSession('Zurück');
    await f.services.tickets.move(id, 'in_progress', f.backend.actor(), { force: true });
    await f.end(id);
    await f.service.tick();
    const ticket = await f.services.tickets.get(id);
    expect(ticket.status).toBe('in_progress');
    expect(ticket.blockedReason ?? null).toBeNull();
    expect((await f.service.stores.state.read()).merges).toEqual({});
  });

  it('Auftrag in Phase „Session“ ohne Merge-Lauf gilt als ungelöst', async () => {
    f = await DispatchFixture.create();
    const id = await inMergeSession('Hängt');
    // Lauf verschwindet ohne Auswertung (z. B. Absturz zwischen Auswertung und Zustand)
    await f.service.stores.runs.remove((await f.run(id)).id);
    await f.service.tick();
    const ticket = await f.services.tickets.get(id);
    expect(ticket.status).toBe('blocked');
    expect(ticket.blockedReason).toBe('Merge: Die Merge-Session endete ohne Auswertung.');
    expect((await f.service.stores.state.read()).merges).toEqual({});
  });

  it('fachlicher Fehler beim Auswerten der Merge-Session blockiert statt liegenzubleiben', async () => {
    f = await DispatchFixture.create();
    const id = await inMergeSession('Kaputt');
    vi.spyOn(BranchMerger.prototype, 'contains').mockImplementation(() => {
      throw new HarnessError('git-failed', 'Git kaputt');
    });
    await f.end(id);
    await f.service.tick();
    const ticket = await f.services.tickets.get(id);
    expect(ticket.status).toBe('blocked');
    expect(ticket.blockedReason).toBe('Merge: Die Merge-Session ließ sich nicht auswerten: Git kaputt');
    expect((await f.service.stores.state.read()).merges).toEqual({});
  });

  it('nach erfolgreichem Merge gehen Merge-blockierte Tickets des Worktrees wieder auf Erledigt', async () => {
    f = await DispatchFixture.create({ wip: 2, joinWorktrees: 'sameSpec' });
    const [first, second] = await sharedDone();
    f.repo.git('checkout', '-q', '--detach');
    await f.service.tick();
    expect((await f.services.tickets.get(first)).status).toBe('blocked');
    expect((await f.services.tickets.get(second)).status).toBe('blocked');
    f.repo.git('checkout', '-q', '-');
    const report = await f.service.retryMerge(first);
    expect(report.merged.sort()).toEqual([first, second].sort());
    expect(f.repo.git('log', '-1', '--format=%s')).toBe(`Merge ${first}, ${second} (ticket/${first}-erstes)`);
    expect(await f.services.tickets.get(second)).toMatchObject({ status: 'done', branch: null });
    expect((await f.comments(second)).at(-1)).toMatch(/^\[dispatch\] Gemergt in [\s\S]*wieder auf Erledigt/);
  });

  it('Abbruch nach dem Merge (Branch schon gelöscht): der nächste Tick schließt den Auftrag ab statt zu blockieren', async () => {
    f = await DispatchFixture.create();
    const id = await doneByAgent('Abbruch');
    const branch = `ticket/${id}-abbruch`;
    // Wie ein Verbindungsabriss: Branch und Worktree sind weg, das Leeren am Ticket scheitert
    const update = f.services.tickets.update.bind(f.services.tickets);
    const spy = vi.spyOn(f.services.tickets, 'update').mockImplementationOnce(async (...args) => {
      if (args[1].branch === null) throw new Error('Connection not yet open.');
      return update(...args);
    });
    const first = await f.service.tick();
    expect(first.merged).toEqual([]);
    expect(f.repo.git('branch', '--list', branch)).toBe('');
    const merge = f.repo.git('log', '-1', '--format=%h');
    spy.mockRestore();
    const report = await f.service.tick();
    expect(report.merged).toEqual([id]);
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'done', branch: null, worktree: null, blockedReason: null });
    expect((await f.comments(id)).at(-1)).toMatch(new RegExp(`^\\[dispatch\\] Gemergt in ${mainBranch()} \\(${merge}\\)\\.`));
    expect(f.repo.git('log', '-1', '--format=%h')).toBe(merge);
    expect((await f.service.stores.state.read()).merges).toEqual({});
  });

  it('Gemergt-Kommentar meldet unvollständiges Aufräumen statt „aufgeräumt“', async () => {
    f = await DispatchFixture.create();
    const id = await doneByAgent('Rest');
    const worktree = (await f.services.tickets.get(id)).worktree!;
    writeFileSync(join(worktree, 'rest.txt'), 'x');
    const report = await f.service.tick();
    expect(report.merged).toEqual([id]);
    const comment = (await f.comments(id)).at(-1)!;
    expect(comment).toMatch(/^\[dispatch\] Gemergt in .* Aufräumen unvollständig:\n.*ungesicherte Änderungen/);
    expect(comment).not.toMatch(/aufgeräumt/);
  });

  it('weist Git den Merge wegen überschneidender lokaler Änderungen ab, wartet der Auftrag statt zu blockieren', async () => {
    f = await DispatchFixture.create();
    commitOnMain('cfg.yaml', 'alt');
    const id = await doneByAgent('Sprache', 'cfg.yaml', 'ticket');
    // Die Änderung entsteht erst zwischen Vorprüfung und Merge
    vi.spyOn(BranchMerger.prototype, 'merge').mockImplementation(() => {
      writeFileSync(join(f.repo.path, 'cfg.yaml'), 'lokal');
      return { kind: 'refused', message: 'Fehler: lokale Änderungen würden überschrieben:\n\tcfg.yaml\nAbbruch' };
    });
    await f.service.tick();
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'done', blockedReason: null });
    expect((await f.comments(id)).at(-1)).toBe('[dispatch] Merge wartet auf den Hauptordner: lokale Änderungen in cfg.yaml, die der Branch ebenfalls ändert – gemergt wird automatisch, sobald das erledigt ist.');
    expect((await f.service.stores.state.read()).merges[id]).toMatchObject({ phase: 'waiting' });
  });

  it('abgewiesener Merge ohne lokale Überschneidung blockiert mit den Dateien aus der Git-Meldung', async () => {
    f = await DispatchFixture.create();
    const id = await doneByAgent('Abweisung');
    vi.spyOn(BranchMerger.prototype, 'merge').mockReturnValue({ kind: 'refused', message: 'error: The following untracked working tree files would be overwritten by merge:\n\tx.txt\nAborting' });
    await f.service.tick();
    const ticket = await f.services.tickets.get(id);
    expect(ticket.blockedReason).toBe('Merge: Git lehnt den Merge ab, er würde lokale Änderungen überschreiben: x.txt');
    expect((await f.comments(id)).at(-1)).toBe('[dispatch] Merge nicht möglich: Git lehnt den Merge ab, er würde lokale Änderungen überschreiben: x.txt');
  });

  it('ausgeschaltet oder außerhalb des Zeitfensters: keine Merge-Session und nach dem Konflikt kein weiterer Merge-Versuch', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Aus');
    await f.service.tick();
    commitOnMain();
    await f.finishAsAgent(id, 'feature.txt', 'ticket');
    await f.end(id);
    await f.service.setEnabled(false);
    const merge = vi.spyOn(BranchMerger.prototype, 'merge');
    await f.service.tick();
    expect(merge).toHaveBeenCalledTimes(1);
    expect(await mergeRuns()).toHaveLength(0);
    expect((await f.service.stores.state.read()).merges[id]).toMatchObject({ phase: 'pending', sessionUsed: false, conflictSeen: true });
    await f.service.tick();
    expect(merge).toHaveBeenCalledTimes(1);
    // „Erneut mergen“ setzt den Merker zurück: genau ein neuer Versuch
    expect((await f.service.retryMerge(id)).result).toMatchObject({ outcome: 'pending', text: expect.stringContaining('Merge-Session wartet') });
    expect(merge).toHaveBeenCalledTimes(2);
    expect(await mergeRuns()).toHaveLength(0);
    // Eingeschaltet, aber außerhalb des Zeitfensters: weiterhin kein Versuch
    await f.service.setEnabled(true);
    const store = new DispatchConfigStore(f.home);
    const now = f.now.getHours() * 60 + f.now.getMinutes();
    const hhmm = (m: number) => `${String(Math.floor((m % 1440) / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`;
    await store.write({ ...(await store.read()).raw, window: { from: hhmm(now + 120), to: hhmm(now + 180) } });
    await f.service.tick();
    expect(merge).toHaveBeenCalledTimes(2);
    expect(await mergeRuns()).toHaveLength(0);
    // Fenster offen: Merge-Versuch und Merge-Session
    await store.write({ ...(await store.read()).raw, window: null });
    await f.service.tick();
    expect(merge).toHaveBeenCalledTimes(3);
    expect(await mergeRuns()).toHaveLength(1);
  });

  it('während einer Pause startet keine Merge-Session; danach schon', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Pause');
    await f.service.tick();
    commitOnMain();
    await f.finishAsAgent(id, 'feature.txt', 'ticket');
    await f.end(id);
    await f.service.pause(30);
    await f.service.tick();
    expect(await mergeRuns()).toHaveLength(0);
    expect((await f.service.stores.state.read()).merges[id]).toMatchObject({ phase: 'pending', sessionUsed: false, conflictSeen: true });
    // Während der Pause kein weiterer Merge-Versuch im Hauptordner
    const merge = vi.spyOn(BranchMerger.prototype, 'merge');
    await f.service.tick();
    expect(merge).not.toHaveBeenCalled();
    await f.service.resume();
    await f.service.tick();
    expect(await mergeRuns()).toHaveLength(1);
  });

  it('leere Konfliktliste heißt „unbekannten Dateien“', async () => {
    f = await DispatchFixture.create();
    const id = await doneByAgent('Leer');
    vi.spyOn(BranchMerger.prototype, 'merge').mockReturnValue({ kind: 'conflict', files: [] });
    await f.service.tick();
    expect((await f.comments(id)).at(-1)).toMatch(/^\[dispatch\] Merge-Konflikt in unbekannten Dateien beim Merge in /);
  });

  it('Git-Meldung ohne Dateiliste steht zusammengefasst in einer Zeile im Blockiert-Grund', async () => {
    f = await DispatchFixture.create();
    const id = await doneByAgent('Zeilen');
    vi.spyOn(BranchMerger.prototype, 'merge').mockReturnValue({ kind: 'refused', message: 'fatal: etwas\n  ging   schief\nHinweis: x' });
    await f.service.tick();
    expect((await f.services.tickets.get(id)).blockedReason).toBe('Merge: Git lehnt den Merge ab: fatal: etwas ging schief Hinweis: x');
  });

  it('nicht nutzbarer Repo-Pfad blockiert mit „Repo-Pfad … nicht nutzbar“', async () => {
    f = await DispatchFixture.create();
    const id = await doneByAgent('Pfad');
    const missing = join(f.dir, 'gibt-es-nicht');
    await f.services.boards.setRepoPath(f.boardId, missing);
    const report = await f.service.tick();
    expect(report.errors).toEqual([]);
    const ticket = await f.services.tickets.get(id);
    expect(ticket.status).toBe('blocked');
    expect(ticket.blockedReason!.startsWith(`Merge: Repo-Pfad ${missing} nicht nutzbar: `)).toBe(true);
  });

  describe('Zielbranch', () => {
    const targets = new TargetBranches();

    // Bereites Ticket mit Zielbranch starten (legt den Zielbranch an)
    async function startedWithTarget(title: string, target = 'release/1'): Promise<string> {
      const id = await f.ready(title, { targetBranch: target });
      await f.service.tick();
      return id;
    }

    // Commit direkt auf den Zielbranch über einen kurzzeitig geöffneten Merge-Worktree
    function commitOnTarget(target: string, file: string, content: string): void {
      const path = targets.openMergeWorktree(f.repo.path, target);
      writeFileSync(join(path, file), content);
      f.repo.git('-C', path, 'add', file);
      f.repo.git('-C', path, 'commit', '-q', '-m', `ziel ${file}`);
      expect(targets.closeMergeWorktree(f.repo.path, path)).toBeNull();
    }

    it('mergt in einen nicht ausgecheckten Zielbranch im Merge-Worktree, ohne den Hauptordner anzufassen', async () => {
      f = await DispatchFixture.create();
      const before = f.repo.git('rev-parse', 'HEAD');
      const id = await startedWithTarget('Login');
      const comments = await f.comments(id);
      expect(comments.join('\n')).toMatch(/^\[dispatch\] Merge: Zielbranch release\/1 neu angelegt von /m);
      // Kein zusätzliches Start-Ereignis: genau ein Start-Kommentar
      expect(comments.filter((c) => c.startsWith('[dispatch] Start'))).toHaveLength(1);
      await f.finishAsAgent(id, 'feature.txt', 'neu');
      await f.end(id);
      const worktree = (await f.services.tickets.get(id)).worktree!;
      const report = await f.service.tick();
      expect(report.merged).toEqual([id]);
      expect(f.repo.git('rev-parse', 'HEAD')).toBe(before);
      expect(existsSync(join(f.repo.path, 'feature.txt'))).toBe(false);
      expect(f.repo.git('show', 'release/1:feature.txt')).toBe('neu');
      expect(f.repo.git('log', '-1', '--format=%s', 'release/1')).toBe(`Merge ${id} „Login“ (ticket/${id}-login)`);
      expect(existsSync(targets.mergeWorktreePath(f.repo.path, 'release/1'))).toBe(false);
      expect(existsSync(worktree)).toBe(false);
      expect(f.repo.git('branch', '--list', `ticket/${id}-login`)).toBe('');
      expect(await f.services.tickets.get(id)).toMatchObject({ status: 'done', branch: null, worktree: null });
      expect((await f.comments(id)).at(-1)).toMatch(/^\[dispatch\] Gemergt in release\/1 \(.*aufgeräumt/);
      expect(f.notices.at(-1)).toMatchObject({ kind: 'merged', ticketId: id, text: expect.stringMatching(/^in release\/1 gemergt/) });
      expect((await f.service.stores.state.read()).merges).toEqual({});
    });

    it('mergeStrategy squash: fasst im Merge-Worktree in den Zielbranch zusammen, ohne den Hauptordner anzufassen', async () => {
      f = await DispatchFixture.create({ mergeStrategy: 'squash' });
      const before = f.repo.git('rev-parse', 'HEAD');
      const id = await startedWithTarget('Login');
      await f.finishAsAgent(id, 'feature.txt', 'neu');
      await f.end(id);
      const ticket = await f.services.tickets.get(id);
      const tip = f.repo.git('rev-parse', `refs/heads/${ticket.branch!}`);
      const targetBefore = f.repo.git('rev-parse', 'refs/heads/release/1');
      expect((await f.service.tick()).merged).toEqual([id]);
      expect(f.repo.git('log', '-1', '--format=%P', 'refs/heads/release/1')).toBe(targetBefore);
      expect(f.repo.git('log', '-1', '--format=%s', 'refs/heads/release/1')).toBe(`${id} „Login“ (squash von ${ticket.branch})`);
      expect(f.repo.git('log', '-1', '--format=%(trailers:key=Harness-Squashed,valueonly)', 'refs/heads/release/1')).toBe(`${ticket.branch} ${tip}`);
      expect(f.repo.git('show', 'refs/heads/release/1:feature.txt')).toBe('neu');
      expect(f.repo.git('branch', '--list', ticket.branch!)).toBe('');
      expect(existsSync(ticket.worktree!)).toBe(false);
      expect(existsSync(targets.mergeWorktreePath(f.repo.path, 'release/1'))).toBe(false);
      expect(f.repo.git('rev-parse', 'HEAD')).toBe(before);
      expect(existsSync(join(f.repo.path, 'feature.txt'))).toBe(false);
      expect(f.repo.git('status', '--porcelain')).toBe('');
      expect(await f.services.tickets.get(id)).toMatchObject({ status: 'done', branch: null, worktree: null });
      expect((await f.comments(id)).at(-1)).toMatch(/^\[dispatch\] Gemergt in release\/1 \(.*aufgeräumt/);
    });

    it('legt einen inzwischen gelöschten Zielbranch beim Merge neu an und kommentiert das', async () => {
      f = await DispatchFixture.create();
      const id = await startedWithTarget('Weg');
      await f.finishAsAgent(id, 'feature.txt', 'neu');
      await f.end(id);
      f.repo.git('branch', '-D', 'release/1');
      expect((await f.service.tick()).merged).toEqual([id]);
      expect(f.repo.git('show', 'release/1:feature.txt')).toBe('neu');
      expect(await f.comments(id)).toContain(`[dispatch] Merge: Zielbranch release/1 neu angelegt von ${mainBranch()}.`);
    });

    it('blockiert, wenn sich der Zielbranch nicht anlegen lässt (Git-Fehler), statt jeden Tick einen Fehler zu melden', async () => {
      f = await DispatchFixture.create();
      const id = await startedWithTarget('Kaputt');
      await f.finishAsAgent(id, 'feature.txt', 'neu');
      await f.end(id);
      // release/1 weg, dafür ein Branch „release“: `git branch release/1` scheitert am Ref-Konflikt
      f.repo.git('branch', '-D', 'release/1');
      f.repo.git('branch', 'release');
      const report = await f.service.tick();
      expect(report.errors).toEqual([]);
      expect(report.merged).toEqual([]);
      const ticket = await f.services.tickets.get(id);
      expect(ticket.status).toBe('blocked');
      expect(ticket.blockedReason).toMatch(/^Merge: Zielbranch release\/1 nicht nutzbar: /);
      expect((await f.service.stores.state.read()).merges).toEqual({});
      expect((await f.service.tick()).errors).toEqual([]);
    });

    it('Konflikt gesehen und Dispatcher aus: kein Merge-Worktree je Tick, der Auftrag wartet', async () => {
      f = await DispatchFixture.create();
      const id = await startedWithTarget('Wartet');
      commitOnTarget('release/1', 'feature.txt', 'haupt');
      await f.finishAsAgent(id, 'feature.txt', 'ticket');
      await f.end(id);
      await f.service.setEnabled(false);
      await f.service.tick();
      expect(await mergeRuns()).toHaveLength(0);
      expect((await f.service.stores.state.read()).merges[id]).toMatchObject({ phase: 'pending', conflictSeen: true });
      const path = targets.mergeWorktreePath(f.repo.path, 'release/1');
      expect(existsSync(path)).toBe(false);
      const open = vi.spyOn(TargetBranches.prototype, 'openMergeWorktree');
      const merge = vi.spyOn(BranchMerger.prototype, 'merge');
      await f.service.tick();
      await f.service.tick();
      expect(open).not.toHaveBeenCalled();
      expect(merge).not.toHaveBeenCalled();
      expect(existsSync(path)).toBe(false);
      expect((await f.services.tickets.get(id)).status).toBe('done');
      expect((await f.service.stores.state.read()).merges[id]).toMatchObject({ phase: 'pending', conflictSeen: true });
    });

    it('nennt den Zielbranch des Tickets im Dispatch-Status, schon vor einer Merge-Session', async () => {
      f = await DispatchFixture.create();
      const id = await startedWithTarget('Wartet');
      commitOnTarget('release/1', 'feature.txt', 'haupt');
      await f.finishAsAgent(id, 'feature.txt', 'ticket');
      await f.end(id);
      await f.service.setEnabled(false);
      await f.service.tick();
      expect((await f.service.stores.state.read()).merges[id]).toMatchObject({ target: null });
      expect((await f.service.status(f.boardId)).merges).toEqual([{ ticketId: id, phase: 'pending', branch: `ticket/${id}-wartet`, target: 'release/1' }]);
    });

    it('ohne Zielbranch am Ticket steht im Dispatch-Status kein Ziel, auch wenn der Auftrag eines kennt', async () => {
      f = await DispatchFixture.create();
      const id = await f.ready('Ohne');
      await f.service.stores.state.update((s) => {
        s.merges[id] = { ticketId: id, branch: 'ticket/x', phase: 'waiting', sessionUsed: true, waitNoted: true, target: mainBranch(), conflictSeen: false, since: '2026-10-22T08:00:00.000Z' };
      });
      expect((await f.service.status(f.boardId)).merges).toEqual([{ ticketId: id, phase: 'waiting', branch: 'ticket/x', target: null }]);
    });

    it('mergt im Hauptordner, wenn dort der Zielbranch ausgecheckt ist', async () => {
      f = await DispatchFixture.create();
      const main = mainBranch();
      const id = await startedWithTarget('Login', main);
      expect((await f.comments(id)).join('\n')).not.toMatch(/neu angelegt/);
      await f.finishAsAgent(id, 'feature.txt', 'neu');
      await f.end(id);
      const worktree = (await f.services.tickets.get(id)).worktree!;
      const report = await f.service.tick();
      expect(report.merged).toEqual([id]);
      expect(readFileSync(join(f.repo.path, 'feature.txt'), 'utf8')).toBe('neu');
      expect(f.repo.git('log', '-1', '--format=%P').split(' ')).toHaveLength(2);
      expect(f.repo.git('log', '-1', '--format=%s')).toBe(`Merge ${id} „Login“ (ticket/${id}-login)`);
      expect(existsSync(worktree)).toBe(false);
      expect(existsSync(targets.mergeWorktreePath(f.repo.path, main))).toBe(false);
      expect(f.repo.git('branch', '--list', `ticket/${id}-login`)).toBe('');
      expect(await f.services.tickets.get(id)).toMatchObject({ status: 'done', branch: null, worktree: null });
      expect((await f.comments(id)).at(-1)).toMatch(new RegExp(`^\\[dispatch\\] Gemergt in ${main} \\(.*aufgeräumt`));
    });

    it('mergt im fremden Worktree des Zielbranches trotz anderer lokaler Änderungen; überschneidende lassen ihn warten', async () => {
      f = await DispatchFixture.create();
      const id = await startedWithTarget('Fremd');
      await f.finishAsAgent(id, 'feature.txt', 'neu');
      await f.end(id);
      const foreign = join(f.repo.base, 'fremd');
      f.repo.git('worktree', 'add', '-q', foreign, 'release/1');
      writeFileSync(join(foreign, 'feature.txt'), 'lokal');
      const tip = f.repo.git('rev-parse', 'release/1');
      expect((await f.service.tick()).merged).toEqual([]);
      expect(await f.services.tickets.get(id)).toMatchObject({ status: 'done', blockedReason: null });
      expect((await f.comments(id)).at(-1)).toMatch(/^\[dispatch\] Merge wartet auf den Worktree .*fremd: lokale Änderungen in feature\.txt, die der Branch ebenfalls ändert/);
      expect(f.repo.git('rev-parse', 'release/1')).toBe(tip);
      expect(existsSync(targets.mergeWorktreePath(f.repo.path, 'release/1'))).toBe(false);
      // Ohne Überschneidung: gemergt wird im fremden Worktree, der samt lokaler Datei stehen bleibt
      rmSync(join(foreign, 'feature.txt'));
      writeFileSync(join(foreign, 'lokal.txt'), 'x');
      expect((await f.service.tick()).merged).toEqual([id]);
      expect(readFileSync(join(foreign, 'feature.txt'), 'utf8')).toBe('neu');
      expect(readFileSync(join(foreign, 'lokal.txt'), 'utf8')).toBe('x');
      expect(f.repo.git('show', 'release/1:feature.txt')).toBe('neu');
    });

    it('wartet bei liegengebliebenem Merge-Worktree mit lokalen Änderungen', async () => {
      f = await DispatchFixture.create();
      const id = await startedWithTarget('Liegen');
      await f.finishAsAgent(id, 'feature.txt', 'neu');
      await f.end(id);
      const path = targets.openMergeWorktree(f.repo.path, 'release/1');
      writeFileSync(join(path, 'rest.txt'), 'x');
      const tip = f.repo.git('rev-parse', 'release/1');
      expect((await f.service.tick()).merged).toEqual([]);
      expect(await f.service.tick()).toMatchObject({ merged: [], errors: [] });
      expect(await f.services.tickets.get(id)).toMatchObject({ status: 'done', blockedReason: null });
      const waits = (await f.comments(id)).filter((c) => c.startsWith('[dispatch] Merge wartet auf den Merge-Worktree'));
      expect(waits).toHaveLength(1);
      expect(waits[0]).toMatch(/: liegengebliebene Änderungen in rest\.txt – /);
      expect(f.repo.git('rev-parse', 'release/1')).toBe(tip);
      expect(existsSync(join(path, 'rest.txt'))).toBe(true);
      // Sauber: der liegengebliebene Merge-Worktree wird wiederverwendet und danach entfernt
      rmSync(join(path, 'rest.txt'));
      expect((await f.service.tick()).merged).toEqual([id]);
      expect(f.repo.git('show', 'release/1:feature.txt')).toBe('neu');
      expect(existsSync(path)).toBe(false);
    });

    it('nennt den Merge-Worktree, wenn sich der eigene Merge dort nicht zurücksetzen lässt', async () => {
      f = await DispatchFixture.create();
      const id = await startedWithTarget('Abbruch');
      await f.finishAsAgent(id, 'feature.txt', 'neu');
      await f.end(id);
      let seen = '';
      vi.spyOn(BranchMerger.prototype, 'merge').mockImplementation((root) => {
        seen = root;
        throw new HarnessError('merge-abort-failed', 'Der Hauptordner steht noch im Merge-Zustand: kaputt. Bitte von Hand aufräumen (git merge --abort).',
          { root, git: 'kaputt', command: 'git merge --abort' });
      });
      expect((await f.service.tick()).errors).toEqual([]);
      const ticket = await f.services.tickets.get(id);
      expect(seen).not.toBe(f.repo.path);
      expect(ticket).toMatchObject({ status: 'blocked' });
      expect(ticket.blockedReason).toBe(`Merge: Der Merge-Worktree ${seen} steht noch im Merge-Zustand: kaputt. Bitte dort von Hand aufräumen (git merge --abort).`);
      // Der Ordner bleibt stehen, damit dort aufgeräumt werden kann
      expect(existsSync(seen)).toBe(true);
    });

    it('prüft nach der Merge-Session den Zielbranch, nicht ein gleichnamiges Tag', async () => {
      f = await DispatchFixture.create();
      const id = await startedWithTarget('Mehrdeutig');
      // Tag auf dem alten Stand des Zielbranches: im Ticket-Branch enthalten, der neue Stand des Zielbranches nicht
      f.repo.git('tag', 'release/1', 'refs/heads/release/1');
      commitOnTarget('release/1', 'feature.txt', 'haupt');
      await f.finishAsAgent(id, 'feature.txt', 'ticket');
      await f.end(id);
      await f.service.tick();
      expect(await f.run(id)).toMatchObject({ kind: 'merge', target: 'release/1' });
      // Die Merge-Session holt den Zielbranch nicht herein
      await f.end(id);
      await f.service.tick();
      const ticket = await f.services.tickets.get(id);
      expect(ticket.status).toBe('blocked');
      expect(ticket.blockedReason).toMatch(/^Merge: Die Merge-Session konnte den Konflikt nicht lösen/);
    });

    it('löst Konflikte über die Merge-Session gegen den Zielbranch', async () => {
      f = await DispatchFixture.create();
      const before = f.repo.git('rev-parse', 'HEAD');
      const id = await startedWithTarget('Konflikt');
      commitOnTarget('release/1', 'feature.txt', 'haupt');
      await f.finishAsAgent(id, 'feature.txt', 'ticket');
      await f.end(id);
      await f.service.tick();
      const ticket = await f.services.tickets.get(id);
      const session = await f.run(id);
      expect(session).toMatchObject({ kind: 'merge', target: 'release/1', cwd: ticket.worktree, branch: ticket.branch });
      expect(session.prompt).toContain('git merge release/1');
      expect((await f.service.stores.state.read()).merges[id]).toMatchObject({ phase: 'session', sessionUsed: true, target: 'release/1' });
      expect((await f.comments(id)).at(-1)).toMatch(/^\[dispatch\] Merge-Konflikt in feature\.txt beim Merge in release\/1 /);
      // Der abgebrochene Merge hinterlässt keinen Merge-Worktree
      expect(existsSync(targets.mergeWorktreePath(f.repo.path, 'release/1'))).toBe(false);
      // Die Merge-Session löst den Konflikt im Worktree; der Hauptordner steht weiter auf seinem Branch (keine Prüfung „Zielbranch gewechselt“)
      f.repo.git('-C', ticket.worktree!, 'merge', '-q', '-s', 'ours', '--no-edit', 'release/1');
      await f.end(id);
      const report = await f.service.tick();
      expect(report.merged).toEqual([id]);
      expect(f.repo.git('show', 'release/1:feature.txt')).toBe('ticket');
      expect(f.repo.git('rev-parse', 'HEAD')).toBe(before);
      expect((await f.comments(id)).at(-1)).toMatch(/^\[dispatch\] Gemergt in release\/1 /);
    });
  });
});
