import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { BoardConfigStore, DispatchConfigStore, HarnessError, type BoardQuestionActor } from '@harness/core';
import { DispatchFixture, OK_OUTPUT } from '../../helpers/DispatchFixture.js';
import { MEMORY_CONFIG, MEMORY_USER } from '../../helpers/MemoryBoardBackend.js';

const HUMAN: BoardQuestionActor = { userId: MEMORY_USER.id, name: 'Florian', kind: 'human', session: null };

describe('DispatchService – starten und auswerten', () => {
  let f: DispatchFixture;
  afterEach(() => f?.dispose());

  it('startet ein bereites Ticket im eigenen Worktree und kommentiert den Start', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Login bauen');
    const report = await f.service.tick();
    expect(report.started).toEqual([id]);
    const ticket = await f.services.tickets.get(id);
    expect(ticket).toMatchObject({ status: 'in_progress', assignee: `junior-${id}`, branch: `ticket/${id}-login-bauen` });
    expect(existsSync(ticket.worktree!)).toBe(true);
    const run = await f.run(id);
    expect(run).toMatchObject({ kind: 'work', cwd: ticket.worktree, branch: ticket.branch, agent: `junior-${id}`, pid: 4242 });
    expect(run.prompt).toContain(`Du bist junior-${id}`);
    expect(run.args).toEqual(expect.arrayContaining(['-p', '--model', 'opus', '--effort', 'high']));
    expect(f.launcher.launched[0]).toMatchObject({ runId: run.id });
    expect(f.launcher.launched[0].env.HARNESS_TICKET).toBe(id);
    expect((await f.comments(id))[0]).toMatch(new RegExp(`^\\[dispatch\\] Start: an junior-${id} übergeben – Branch ticket/${id}-login-bauen, Profil standard`));
  });

  it('hält WIP-Limit und Opt-in ein und zeigt die Gründe im Status', async () => {
    f = await DispatchFixture.create();
    const first = await f.ready('Erstes', { priority: 'high' });
    const second = await f.ready('Zweites');
    const manual = await f.ready('Ohne Label', { labels: [] });
    const report = await f.service.tick();
    expect(report.started).toEqual([first]);
    expect(report.queued).toEqual([second]);
    const status = await f.service.status(f.boardId);
    expect(status).toMatchObject({ enabled: true, state: 'running', active: 1, wip: 1, boardInScope: true });
    expect(status.runs[0]).toMatchObject({ ticketId: first, kind: 'work', profile: 'standard' });
    expect(status.tickets).toEqual([
      { id: second, title: 'Zweites', reason: 'bereit – wartet auf freien Platz' },
      { id: manual, title: 'Ohne Label', reason: 'Label „auto“ fehlt' },
    ]);
  });

  it('startet nichts, wenn ausgeschaltet, pausiert oder außerhalb des Zeitfensters', async () => {
    f = await DispatchFixture.create({ enabled: false });
    await f.ready('Erstes');
    expect((await f.service.tick()).idle).toBe('Dispatcher ist ausgeschaltet');
    await f.service.setEnabled(true);
    await f.service.pause(30);
    expect((await f.service.tick()).idle).toMatch(/^Pausiert bis \d\d:\d\d$/);
    expect((await f.service.status()).state).toBe('paused');
    await f.service.resume();
    f.now = new Date(2026, 9, 15, 12, 0);
    const store = new DispatchConfigStore(f.home);
    await store.write({ ...(await store.read()).raw, window: { from: '18:00', to: '07:00' } });
    expect((await f.service.tick()).idle).toBe('Außerhalb des Zeitfensters 18:00–07:00');
    expect((await f.service.status()).state).toBe('outside');
  });

  it('Frage: zurück nach Geplant, nach der Entscheidung weiter im selben Worktree mit Entscheidung', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Rabatte');
    await f.service.tick();
    const branch = (await f.services.tickets.get(id)).branch;
    const question = await f.services.questions.create('ARC', { title: 'Brutto oder netto?', ticketIds: [id] }, { ...HUMAN, name: `junior-${id}`, kind: 'session', session: 's' });
    await f.end(id);
    const report = await f.service.tick();
    expect(report.evaluated).toEqual([{ ticketId: id, outcome: 'question' }]);
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'planned', assignee: null, branch });
    expect((await f.comments(id)).at(-1)).toMatch(/^\[dispatch\] Frage: wartet auf ARC-F0001/);
    expect(f.notices.map((n) => n.kind)).toEqual(['question']);
    expect(report.started).toEqual([]);
    await f.services.questions.answer(question.id, { text: 'Brutto', reason: 'Shop zeigt Brutto' }, HUMAN);
    expect((await f.service.tick()).started).toEqual([id]);
    const ticket = await f.services.tickets.get(id);
    expect(ticket).toMatchObject({ status: 'in_progress', branch });
    expect((await f.run(id)).prompt).toContain('ARC-F0001 „Brutto oder netto?“: Brutto (Begründung: Shop zeigt Brutto)');
  });

  it('Abbruch: zurück nach Geplant, eskaliert, beim letzten Versuch Blockiert mit Meldung', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Schwierig');
    await f.service.tick();
    await f.end(id, 'Fehlerausgabe', 2);
    const first = await f.service.tick();
    expect(first.evaluated).toEqual([{ ticketId: id, outcome: 'released' }]);
    expect((await f.comments(id)).find((c) => c.startsWith('[dispatch] Abbruch'))).toMatch(/^\[dispatch\] Abbruch Versuch 1\/2: Claude endete mit Exit-Code 2\.\nDas Ticket geht zurück nach Geplant\./);
    expect(first.started).toEqual([id]);
    expect((await f.comments(id)).at(-1)).toContain('Versuch 2: eskaliert auf schwer');
    await f.end(id, OK_OUTPUT, 0);
    const second = await f.service.tick();
    expect(second.evaluated).toEqual([{ ticketId: id, outcome: 'blocked' }]);
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'blocked', assignee: null, blockedReason: 'Automatische Umsetzung 2× abgebrochen – bitte ansehen' });
    expect(f.notices.at(-1)).toMatchObject({ kind: 'blocked', ticketId: id });
  });

  it('Nutzungslimit: Pause, kein Versuch gezählt', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Limit');
    await f.service.tick();
    await f.end(id, 'Claude usage limit reached', 1);
    const report = await f.service.tick();
    expect(report.evaluated).toEqual([{ ticketId: id, outcome: 'limit' }]);
    expect(report.idle).toMatch(/^Pausiert bis/);
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'planned', assignee: null });
    expect((await f.comments(id)).some((c) => c.startsWith('[dispatch] Abbruch'))).toBe(false);
    expect(f.notices.at(-1)?.kind).toBe('paused');
  });

  it('Sitzungslimit neuerer Claude-Code-Versionen („hit your session limit“): Pause statt Abbruch', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Sitzungslimit');
    await f.service.tick();
    await f.end(id, "You've hit your session limit · resets 12:10pm (Europe/Berlin)", 1);
    const report = await f.service.tick();
    expect(report.evaluated).toEqual([{ ticketId: id, outcome: 'limit' }]);
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'planned', assignee: null });
    expect((await f.comments(id)).some((c) => c.startsWith('[dispatch] Abbruch'))).toBe(false);
  });

  it('Erledigt: legt einen Merge-Auftrag an', async () => {
    f = await DispatchFixture.create({ joinWorktrees: 'sameSpec' });
    const id = await f.ready('Fertig');
    await f.service.tick();
    await f.services.tickets.move(id, 'done', f.agent(id));
    await f.end(id);
    await f.service.tick();
    expect((await f.service.stores.state.read()).merges[id]).toMatchObject({ phase: expect.any(String), branch: `ticket/${id}-fertig` });
  });

  it('ein Worktree je Ticket: Erledigt ohne „finish“ wird im selben Tick gemergt', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Fertig');
    await f.service.tick();
    await f.services.tickets.move(id, 'done', f.agent(id));
    await f.end(id);
    expect((await f.service.tick()).merged).toEqual([id]);
    expect((await f.service.stores.state.read()).merges).toEqual({});
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'done', branch: null, worktree: null });
  });

  it('gibt Tickets zurück, deren Worker ohne Ergebnis verschwunden ist oder die lange nichts tun', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Absturz');
    await f.service.tick();
    f.alive = false;
    const report = await f.service.tick();
    expect(report.evaluated).toEqual([{ ticketId: id, outcome: 'released' }]);
    expect((await f.comments(id)).find((c) => c.startsWith('[dispatch] Abbruch'))).toContain('Worker-Prozess endete ohne Ergebnis');
    f.alive = true;
    // Lauf-Datei verloren, Ticket hängt beim Agenten
    for (const r of await f.service.stores.runs.list()) await f.service.stores.runs.remove(r.id);
    const store = new DispatchConfigStore(f.home);
    await store.write({ ...(await store.read()).raw, wip: 0 });
    f.advance(200);
    const stale = await f.service.tick();
    expect(stale.evaluated).toEqual([{ ticketId: id, outcome: 'blocked' }]);
    expect((await f.comments(id)).filter((c) => c.startsWith('[dispatch] Abbruch')).at(-1)).toContain('Kein Lebenszeichen seit mehr als 150 min');
  });

  it('gibt das Ticket zurück, wenn der Worker nicht startet; nach zwei Startfehlern Blockiert', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Kein Claude');
    f.launcher.fail = new Error('claude fehlt');
    const report = await f.service.tick();
    expect(report.errors).toEqual([`${id}: claude fehlt`]);
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'planned', assignee: null });
    expect((await f.comments(id)).at(-1)).toMatch(/^\[dispatch\] Abbruch Versuch 1\/2: Worker konnte nicht gestartet werden: claude fehlt/);
    expect(await f.service.stores.runs.list()).toEqual([]);
    const second = await f.service.tick();
    expect(second.errors).toEqual([`${id}: claude fehlt`]);
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'blocked', assignee: null });
    expect((await f.comments(id)).filter((c) => c.startsWith('[dispatch] Abbruch'))).toHaveLength(2);
  });

  it('lässt Tickets in Ruhe, die ein Mensch übernommen hat', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Übernommen');
    await f.service.tick();
    await f.services.tickets.claim(id, 'Florian', f.backend.actor(), { force: true });
    await f.end(id, 'x', 1);
    expect((await f.service.tick()).evaluated).toEqual([{ ticketId: id, outcome: 'taken' }]);
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'in_progress', assignee: 'Florian' });
  });
});

describe('DispatchService – robust gegen DB-Abbruch, Doppelstart und fremde Änderungen', () => {
  let f: DispatchFixture;
  afterEach(() => {
    vi.restoreAllMocks();
    f?.dispose();
  });

  it('Verbindungsfehler bei der Auswertung: Tick bricht ab, der Lauf bleibt für den nächsten Tick', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Netz');
    await f.service.tick();
    await f.end(id, 'x', 1);
    const get = vi.spyOn(f.services.tickets, 'get');
    get.mockRejectedValueOnce(Object.assign(new Error('Socket geschlossen'), { code: 'ESOCKET' }));
    const first = await f.service.tick();
    expect(first.evaluated).toEqual([]);
    expect(first.errors).toEqual(['Verbindung zur Board-Datenbank verloren: Socket geschlossen']);
    expect(await f.service.stores.runs.list()).toHaveLength(1);
    get.mockRejectedValueOnce(new HarnessError('board-db-unreachable', 'Datenbank weg'));
    expect((await f.service.tick()).errors).toEqual(['Datenbank weg']);
    expect(await f.service.stores.runs.list()).toHaveLength(1);
    expect((await f.service.tick()).evaluated).toEqual([{ ticketId: id, outcome: 'released' }]);
  });

  it('fachlicher Fehler bei der Auswertung: protokolliert, Lauf entfernt', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Fachlich');
    await f.service.tick();
    await f.end(id, 'x', 1);
    vi.spyOn(f.services.tickets, 'get').mockRejectedValueOnce(new HarnessError('ticket-invalid', 'Ticket kaputt'));
    const report = await f.service.tick();
    expect(report.evaluated).toEqual([]);
    expect(report.errors).toEqual([`${id}: Ticket kaputt`]);
    expect(await f.service.stores.runs.list()).toEqual([]);
  });

  it('Programmierfehler (TypeError) bei der Auswertung: Lauf entfernt, Fehler im Bericht, Tick läuft weiter', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Bug');
    await f.service.tick();
    await f.end(id, 'x', 1);
    const next = await f.ready('Nächstes');
    vi.spyOn(f.services.tickets, 'get').mockRejectedValueOnce(new TypeError("Cannot read properties of undefined (reading 'x')"));
    const report = await f.service.tick();
    expect(report.errors).toEqual([`${id}: Cannot read properties of undefined (reading 'x')`]);
    expect(report.started).toEqual([next]);
    expect((await f.service.stores.runs.list()).map((r) => r.ticketId)).toEqual([next]);
  });

  it('Verbindungsfehler beim Übernehmen: Tick bricht ab, das Ticket wird nicht übersprungen', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Claim');
    vi.spyOn(f.services.tickets, 'claim').mockRejectedValueOnce(Object.assign(new Error('Socket geschlossen'), { code: 'ESOCKET' }));
    const report = await f.service.tick();
    expect(report.errors).toEqual(['Verbindung zur Board-Datenbank verloren: Socket geschlossen']);
    expect(report.started).toEqual([]);
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'planned', assignee: null });
    expect((await f.service.tick()).started).toEqual([id]);
  });

  it('fachlicher Fehler beim Übernehmen: Ticket übersprungen, kein Fehler im Bericht', async () => {
    f = await DispatchFixture.create();
    await f.ready('Claim fachlich');
    vi.spyOn(f.services.tickets, 'claim').mockRejectedValueOnce(new HarnessError('conflict', 'geändert'));
    const report = await f.service.tick();
    expect(report).toMatchObject({ started: [], errors: [] });
  });

  it('PID-Speichern wird einmal wiederholt, dann hat der Lauf die PID', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('PID zweimal');
    const setPid = vi.spyOn(f.service.stores.runs, 'setPid').mockRejectedValueOnce(new Error('Datei gesperrt'));
    await f.service.tick();
    expect(setPid).toHaveBeenCalledTimes(2);
    expect(await f.run(id)).toMatchObject({ pid: 4242 });
  });

  it('Ticket erledigt, Assignee leer: Merge-Auftrag wird angelegt', async () => {
    f = await DispatchFixture.create({ joinWorktrees: 'sameSpec' });
    const id = await f.ready('Leer erledigt');
    await f.service.tick();
    await f.end(id);
    await f.services.tickets.update(id, { status: 'done', assignee: null }, f.backend.actor(), { force: true });
    const report = await f.service.tick();
    expect(report.evaluated).toEqual([{ ticketId: id, outcome: 'done' }]);
    // Gebündelt und ohne `finish` ist der Worktree nicht „bereit“: der Merge-Dienst lässt den Auftrag im selben Tick warten
    expect((await f.service.stores.state.read()).merges[id]).toMatchObject({ ticketId: id, phase: 'waiting' });
  });

  it('gibt eine hängende Übernahme (Geplant mit Agent, ohne Lauf) nach 5 min frei, ohne Versuch zu zählen', async () => {
    f = await DispatchFixture.create({ wip: 0 });
    const id = await f.ready('Hängt');
    await f.services.tickets.claim(id, `junior-${id}`, f.agent(id), { status: 'planned' });
    f.advance(4);
    await f.service.tick();
    expect((await f.services.tickets.get(id)).assignee).toBe(`junior-${id}`);
    f.advance(2);
    const report = await f.service.tick();
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'planned', assignee: null });
    expect(report.queued).toEqual([id]);
    expect((await f.comments(id)).some((c) => c.startsWith('[dispatch] Abbruch'))).toBe(false);
  });

  it('scheitert nach einem Startfehler auch das Zurückgeben, bleibt das Ergebnis der Startfehler; das Ticket löst sich später', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Doppelt kaputt');
    f.launcher.fail = new Error('claude fehlt');
    vi.spyOn(f.services.tickets, 'release').mockRejectedValueOnce(new HarnessError('conflict', 'geändert'));
    const report = await f.service.tick();
    expect(report.errors).toEqual([`${id}: claude fehlt`]);
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'in_progress', assignee: `junior-${id}` });
    f.advance(151);
    expect((await f.service.tick()).evaluated).toEqual([{ ticketId: id, outcome: 'blocked' }]);
  });

  it('kann die PID nicht gespeichert werden, bleibt der Lauf (ohne PID) und das Ticket beim Agenten', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('PID');
    vi.spyOn(f.service.stores.runs, 'setPid').mockRejectedValue(new Error('Datei gesperrt'));
    const report = await f.service.tick();
    expect(report).toMatchObject({ started: [id], errors: [] });
    expect(await f.run(id)).toMatchObject({ pid: null });
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'in_progress', assignee: `junior-${id}` });
    f.advance(3);
    expect((await f.service.tick()).evaluated).toEqual([{ ticketId: id, outcome: 'released' }]);
  });

  it('scheitert der Start-Kommentar, läuft der Worker weiter und wird kein zweites Mal gestartet', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Kommentar');
    const add = f.services.comments.add.bind(f.services.comments);
    vi.spyOn(f.services.comments, 'add').mockImplementation(async (ticketId, body, actor) => {
      if (body.startsWith('[dispatch] Start')) throw new Error('Verbindung weg');
      return add(ticketId, body, actor);
    });
    const report = await f.service.tick();
    expect(report).toMatchObject({ started: [id], errors: [] });
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'in_progress', assignee: `junior-${id}` });
    expect(await f.service.stores.runs.list()).toHaveLength(1);
    expect((await f.service.tick()).started).toEqual([]);
    expect(f.launcher.launched).toHaveLength(1);
  });

  it('prüft vor der Übernahme erneut: zwischenzeitlich nicht mehr bereit oder geändert, dann keine Übernahme', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Mensch ändert');
    const human = f.backend.actor();
    const get = f.services.tickets.get.bind(f.services.tickets);
    const spy = vi.spyOn(f.services.tickets, 'get');
    spy.mockImplementationOnce(async (ticketId: string) => {
      await f.services.tickets.update(ticketId, { labels: [] }, human);
      return get(ticketId);
    });
    expect((await f.service.tick()).started).toEqual([]);
    expect((await f.services.tickets.get(id)).assignee).toBeNull();
    await f.services.tickets.update(id, { labels: ['auto'] }, human);
    spy.mockImplementationOnce(async (ticketId: string) => {
      const detail = await get(ticketId);
      await f.services.tickets.update(ticketId, { title: 'Umbenannt' }, human);
      return detail;
    });
    expect((await f.service.tick()).started).toEqual([]);
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'planned', assignee: null, title: 'Umbenannt' });
    expect(f.launcher.launched).toEqual([]);
  });

  it('Lauf ohne Ergebnis weit über dem Zeitlimit gilt als verschwunden, auch wenn die PID lebt', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Hänger');
    await f.service.tick();
    const { run } = await f.run(id);
    f.advance(run.timeoutMinutes + 9);
    expect((await f.service.tick()).evaluated).toEqual([]);
    f.advance(2);
    expect((await f.service.tick()).evaluated).toEqual([{ ticketId: id, outcome: 'released' }]);
    expect((await f.comments(id)).find((c) => c.startsWith('[dispatch] Abbruch'))).toContain('nach dem Zeitlimit noch kein Ergebnis');
  });

  it('lässt Tickets auf fremden Boards in Ruhe', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Fremd');
    await f.service.tick();
    for (const r of await f.service.stores.runs.list()) await f.service.stores.runs.remove(r.id);
    await new BoardConfigStore(f.home).write({ ...MEMORY_CONFIG, userId: '22222222-2222-2222-2222-222222222222', activeBoard: f.boardId });
    f.advance(200);
    expect((await f.service.tick()).evaluated).toEqual([]);
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'in_progress', assignee: `junior-${id}` });
  });

  it('übernimmt kein Ticket, dessen Branch es auf diesem Rechner nicht gibt', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Anderer Rechner', { branch: 'ticket/ARC-0001-woanders' });
    const report = await f.service.tick();
    expect(report).toMatchObject({ started: [], errors: [] });
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'planned', assignee: null });
    expect(f.launcher.launched).toEqual([]);
  });

  it('Erledigt durch einen Menschen nach Übernahme: kein Merge-Auftrag', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Mensch fertig');
    await f.service.tick();
    await f.services.tickets.claim(id, 'Florian', f.backend.actor(), { force: true });
    await f.services.tickets.move(id, 'done', f.backend.actor());
    await f.end(id);
    expect((await f.service.tick()).evaluated).toEqual([{ ticketId: id, outcome: 'taken' }]);
    expect((await f.service.stores.state.read()).merges[id]).toBeUndefined();
  });
});

describe('DispatchService – Endreview', () => {
  let f: DispatchFixture;
  let extra: string | null = null;
  afterEach(() => {
    vi.restoreAllMocks();
    f?.dispose();
    if (extra) rmSync(extra, { recursive: true, force: true });
    extra = null;
  });

  it('nie zwei Worker im selben Worktree: das zweite Ticket derselben Spec wartet, bis der Worktree frei ist', async () => {
    f = await DispatchFixture.create({ wip: 2, joinWorktrees: 'sameSpec' });
    const first = await f.ready('Erstes', { spec: '001-x', specTask: 'T001', touches: ['a'] });
    const second = await f.ready('Zweites', { spec: '001-x', specTask: 'T002', touches: ['b'] });
    const report = await f.service.tick();
    expect(report.started).toEqual([first]);
    expect(report.errors).toEqual([]);
    expect(await f.services.tickets.get(second)).toMatchObject({ status: 'planned', assignee: null, branch: null });
    expect(await f.comments(second)).toEqual([]);
    expect((await f.service.status(f.boardId)).tickets).toEqual([{ id: second, title: 'Zweites', reason: `wartet – Worktree belegt durch ${first}` }]);
    // Nach dem Ende des ersten Laufs tritt das zweite Ticket dem Worktree bei
    await f.finishAsAgent(first, 'a.txt', 'a');
    await f.end(first);
    expect((await f.service.tick()).started).toEqual([second]);
    expect((await f.services.tickets.get(second)).branch).toBe((await f.services.tickets.get(first)).branch);
  });

  it('Fortsetzung wartet ohne Übernahme, solange auf ihrem Branch ein anderer Lauf aktiv ist', async () => {
    f = await DispatchFixture.create({ wip: 2 });
    const id = await f.ready('Fortsetzung');
    await f.service.tick();
    const record = await f.run(id);
    await f.end(id, 'x', 1);
    await f.service.tick();
    // Lauf entfernen und einen fremden Lauf (z. B. Merge-Session) auf demselben Branch eintragen
    for (const r of await f.service.stores.runs.list()) await f.service.stores.runs.remove(r.id);
    await f.services.tickets.release(id, f.backend.actor(), { status: 'planned', force: true });
    await f.service.stores.runs.create({ ...record, id: 'ARC-0099-20261015-120000-merge', ticketId: 'ARC-0099', kind: 'merge' });
    const report = await f.service.tick();
    expect(report.started).toEqual([]);
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'planned', assignee: null, branch: record.branch });
    expect((await f.service.status(f.boardId)).tickets).toEqual([{ id, title: 'Fortsetzung', reason: 'wartet – Worktree belegt durch ARC-0099' }]);
  });

  it('„rate limit“ im Ergebnistext eines erfolgreichen Laufs ist kein Nutzungslimit', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Limit im Text');
    await f.service.tick();
    await f.end(id, JSON.stringify({ type: 'result', subtype: 'success', is_error: false, result: 'Usage limit reached-Anzeige und rate limit gebaut' }), 0);
    const report = await f.service.tick();
    expect(report.evaluated).toEqual([{ ticketId: id, outcome: 'released' }]);
    expect(report.idle).toBeNull();
    expect((await f.comments(id)).some((c) => c.startsWith('[dispatch] Pause'))).toBe(false);
    expect((await f.comments(id)).some((c) => c.startsWith('[dispatch] Abbruch Versuch 1/2'))).toBe(true);
  });

  it('ab der dritten Limit-Pause in Folge zählt das Limit als Versuch', async () => {
    f = await DispatchFixture.create({ maxAttempts: 3 });
    const id = await f.ready('Immer Limit');
    await f.service.tick();
    for (const expected of ['limit', 'limit']) {
      await f.end(id, 'Claude usage limit reached', 1);
      expect((await f.service.tick()).evaluated).toEqual([{ ticketId: id, outcome: expected }]);
      await f.service.resume();
      expect((await f.service.tick()).started).toEqual([id]);
    }
    await f.end(id, 'Claude usage limit reached', 1);
    expect((await f.service.tick()).evaluated).toEqual([{ ticketId: id, outcome: 'released' }]);
    const comments = await f.comments(id);
    expect(comments.filter((c) => c.startsWith('[dispatch] Pause: Nutzungslimit'))).toHaveLength(2);
    expect(comments.find((c) => c.startsWith('[dispatch] Abbruch'))).toMatch(/^\[dispatch\] Abbruch Versuch 1\/3: Nutzungslimit zum 3\. Mal in Folge\./);
  });

  it('Claude Code nicht gefunden: Pause ohne Versuch, Kommentar und Meldung', async () => {
    f = await DispatchFixture.create();
    const id = await f.ready('Ohne Claude');
    await f.service.tick();
    await f.end(id, 'spawn claude ENOENT', 127);
    const report = await f.service.tick();
    expect(report.evaluated).toEqual([{ ticketId: id, outcome: 'claude-missing' }]);
    expect(report.idle).toMatch(/^Pausiert bis/);
    expect(await f.services.tickets.get(id)).toMatchObject({ status: 'planned', assignee: null });
    const comments = await f.comments(id);
    expect(comments.some((c) => c.startsWith('[dispatch] Abbruch'))).toBe(false);
    expect(comments.at(-1)).toMatch(/^\[dispatch\] Pause: Claude Code wurde nicht gefunden \(Befehl „claude“\)/);
    expect(f.notices.at(-1)).toMatchObject({ kind: 'paused', ticketId: id, text: 'Claude Code nicht gefunden – Pause 60 min' });
  });

  it('ein Board mit unbrauchbarem Repo-Pfad bricht den Tick nicht ab; das nächste Board wird bedient', async () => {
    f = await DispatchFixture.create();
    extra = mkdtempSync(join(tmpdir(), 'harness-kein-repo-'));
    const actor = f.backend.actor();
    const bad = await f.services.boards.create('ARC', 'Kaputt', actor);
    await f.services.boards.setRepoPath(bad.id, extra);
    const created = await f.services.tickets.create(bad.id, { title: 'Woanders', acceptance: 'ok', labels: ['auto'], needsPlanning: false }, actor);
    await f.services.tickets.update(created.id, { plan: 'p', handbook: 'h', status: 'planned' }, actor);
    const store = new DispatchConfigStore(f.home);
    await store.write({ ...(await store.read()).raw, wip: 2, boards: [bad.id, f.boardId] });
    const good = await f.ready('Hier');
    const report = await f.service.tick();
    expect(report.started).toEqual([good]);
    expect(report.errors).toHaveLength(1);
    expect(report.errors[0]).toMatch(new RegExp(`^Board ${bad.id}: `));
    expect(await f.services.tickets.get(created.id)).toMatchObject({ status: 'planned', assignee: null });
  });
});
