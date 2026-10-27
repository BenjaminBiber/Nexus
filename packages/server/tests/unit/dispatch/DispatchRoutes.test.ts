import { writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  BoardConfigStore, DISPATCH_PROMPT_DEFAULTS, DISPATCH_PROMPT_PLACEHOLDERS, DispatchStateStore, PROMPT_KINDS, RunStore,
  type BoardQuestionActor, type DispatchPromptKind,
} from '@harness/core';
import { TestServer } from '../../helpers/TestServer.js';
import { MEMORY_CONFIG, MEMORY_USER } from '../../helpers/MemoryBoardBackend.js';
import { TempRepo } from '../../../../core/tests/unit/helpers/TempRepo.js';
import type { MemoryBoardBackend } from '../../helpers/MemoryBoardBackend.js';

// Knappe eigene work-Vorlage, die alle Pflichtteile erfüllt
const WORK = [
  '{{agent}} setzt {{ticketId}} („{{title}}“) um.',
  'Fertig: harness board done {{ticketId}} --by {{agent}}',
  'Blockiert: harness board set {{ticketId}} --status blocked',
  'Frage: harness board question add "…" --ticket {{ticketId}}',
].join('\n');

describe('DispatchRoutes', () => {
  let server: TestServer;
  let backend: MemoryBoardBackend;

  beforeEach(async () => {
    ({ server, backend } = await TestServer.startWithBoard());
  });
  afterEach(() => server.dispose());

  it('liefert ohne Datei das Beispiel und speichert geprüft', async () => {
    const empty = (await server.get('/api/dispatch/config')).json;
    expect(empty).toMatchObject({ exists: false, error: null, config: { enabled: false, optInLabel: 'auto' } });
    expect(Object.keys(empty.raw.profiles)).toEqual(['klein', 'standard', 'schwer']);
    const saved = await server.put('/api/dispatch/config', { ...empty.raw, wip: 2 });
    expect(saved.json).toMatchObject({ exists: true, config: { wip: 2 } });
    const bad = await server.put('/api/dispatch/config', { wip: -1 });
    expect(bad.status).toBe(422);
    expect(bad.json.code).toBe('dispatch-config-invalid');
    expect((await server.put('/api/dispatch/config', [])).status).toBe(422);
  });

  it('Speichern mit ein- bzw. ausgeschalteter Recherche-Automatik setzt bzw. vergisst den Zeitpunkt, ab dem Fragen zählen', async () => {
    const state = new DispatchStateStore(server.home);
    const raw = (await server.get('/api/dispatch/config')).json.raw;
    await server.put('/api/dispatch/config', { ...raw, wip: 2 });
    expect((await state.read()).researchAuto.since).toBeNull();
    await server.put('/api/dispatch/config', { ...raw, research: { auto: true } });
    const since = (await state.read()).researchAuto.since;
    expect(since).not.toBeNull();
    await server.put('/api/dispatch/config', { ...raw, research: { auto: true, wip: 2 } });
    expect((await state.read()).researchAuto.since).toBe(since);
    await server.put('/api/dispatch/config', { ...raw, research: { auto: false } });
    expect((await state.read()).researchAuto.since).toBeNull();
    expect((await server.put('/api/dispatch/config', { ...raw, research: { wip: 6 } })).json.code).toBe('dispatch-config-invalid');
  });

  it('Speichern mit ein- bzw. ausgeschalteter Planungs-Automatik setzt bzw. vergisst den Zeitpunkt; der Status nennt sie', async () => {
    const state = new DispatchStateStore(server.home);
    const raw = (await server.get('/api/dispatch/config')).json.raw;
    expect((await server.get('/api/dispatch/status')).json.planningAuto).toMatchObject({ auto: false, wip: 1, active: 0, since: null });
    await server.put('/api/dispatch/config', { ...raw, planning: { auto: true, wip: 2 } });
    const since = (await state.read()).planningAuto.since;
    expect(since).not.toBeNull();
    expect((await server.get('/api/dispatch/status')).json.planningAuto).toMatchObject({ auto: true, wip: 2, since });
    await server.put('/api/dispatch/config', { ...raw, planning: { auto: true, wip: 3 } });
    expect((await state.read()).planningAuto.since).toBe(since);
    await server.put('/api/dispatch/config', { ...raw, planning: { auto: false } });
    expect((await state.read()).planningAuto.since).toBeNull();
    expect((await server.put('/api/dispatch/config', { ...raw, planning: { wip: 6 } })).json.code).toBe('dispatch-config-invalid');
  });

  it('schaltet ein und aus, pausiert und setzt fort', async () => {
    expect((await server.post('/api/dispatch/enable')).json).toMatchObject({ enabled: true });
    const paused = (await server.post('/api/dispatch/pause', { minutes: 15 })).json;
    expect(paused.state).toBe('paused');
    expect(paused.pausedUntil).not.toBeNull();
    expect((await server.post('/api/dispatch/pause', { minutes: 0 })).status).toBe(422);
    expect((await server.post('/api/dispatch/resume')).json.pausedUntil).toBeNull();
    expect((await server.post('/api/dispatch/disable')).json).toMatchObject({ enabled: false, state: 'off', message: 'Dispatcher ist ausgeschaltet' });
  });

  it('prüft boardId und minutes vor der Aktion', async () => {
    const bad = await server.post('/api/dispatch/enable', { boardId: 'x' });
    expect(bad.status).toBe(422);
    expect(bad.json.code).toBe('id-invalid');
    expect((await server.get('/api/dispatch/status')).json.enabled).toBe(false);
    expect((await server.post('/api/dispatch/pause', { minutes: 15, boardId: 0 })).status).toBe(422);
    expect((await server.get('/api/dispatch/status')).json.pausedUntil).toBeNull();
    expect((await server.post('/api/dispatch/pause', { minutes: 'viel' })).status).toBe(422);
    expect((await server.get('/api/dispatch/status')).json.pausedUntil).toBeNull();
  });

  it('meldet ein unbekanntes Board mit board-not-found, bevor etwas passiert', async () => {
    const status = await server.get('/api/dispatch/status?boardId=99');
    expect(status.status).toBe(404);
    expect(status.json).toMatchObject({ code: 'board-not-found', message: 'Board 99 gibt es nicht.' });
    const run = await server.post('/api/dispatch/run', { boardId: 99 });
    expect(run.status).toBe(404);
    expect(run.json.code).toBe('board-not-found');
    expect((await server.get('/api/dispatch/status')).json.lastTick).toBeNull();
    expect((await server.post('/api/dispatch/enable', { boardId: 99 })).status).toBe(404);
    expect((await server.get('/api/dispatch/status')).json.enabled).toBe(false);
  });

  it('zeigt die Bereitschaft der Geplant-Tickets eines Boards', async () => {
    const actor = backend.actor();
    await backend.services.projects.create('ARC', 'Arches', actor);
    const board = await backend.services.boards.create('ARC', 'Sprint', actor);
    const t = await backend.services.tickets.create(board.id, { title: 'Ohne Plan', labels: ['auto'], needsPlanning: false }, actor);
    await backend.services.tickets.update(t.id, { status: 'planned' }, actor);
    const status = (await server.get(`/api/dispatch/status?boardId=${board.id}`)).json;
    expect(status).toMatchObject({ boardId: board.id, tickets: [{ id: t.id, title: 'Ohne Plan', reason: 'Kein Plan' }] });
    expect((await server.post('/api/dispatch/run', { boardId: board.id })).json.lastTick).not.toBeNull();
    const merge = await server.post(`/api/dispatch/merge/${t.id}`);
    expect(merge.status).toBe(422);
    expect(merge.json.code).toBe('merge-not-possible');
  });

  it('„Erneut mergen“ liefert das Ergebnis für das Ticket und lehnt Tickets fremder Boards ab', async () => {
    const actor = backend.actor();
    await backend.services.projects.create('ARC', 'Arches', actor);
    const own = await backend.services.boards.create('ARC', 'Sprint', actor);
    const mine = await backend.services.tickets.create(own.id, { title: 'Eigenes', acceptance: 'ok', needsPlanning: false }, actor);
    await backend.services.tickets.update(mine.id, { status: 'done', branch: 'ticket/x' }, actor, { force: true });
    const merge = await server.post(`/api/dispatch/merge/${mine.id}`);
    expect(merge.status).toBe(200);
    expect(merge.json.result).toEqual({ outcome: 'blocked', text: `${mine.id}: Merge blockiert – Das Board hat keinen Repo-Pfad.` });
    expect(merge.json).toHaveProperty('enabled');

    const other = { id: '22222222-2222-2222-2222-222222222222', name: 'Kollege' };
    await backend.db.transaction((u) => u.users.upsert(other));
    const foreign = await backend.services.boards.create('ARC', 'Fremd', other);
    const theirs = await backend.services.tickets.create(foreign.id, { title: 'Fremdes', acceptance: 'ok', needsPlanning: false }, other);
    await backend.services.tickets.update(theirs.id, { status: 'done', branch: 'ticket/y' }, other, { force: true });
    const refused = await server.post(`/api/dispatch/merge/${theirs.id}`);
    expect(refused.status).toBe(422);
    expect(refused.json).toMatchObject({ code: 'merge-not-possible', message: `${theirs.id} liegt auf dem Board eines anderen Benutzers.` });
    expect((await backend.services.tickets.get(theirs.id)).status).toBe('done');
  });

  it('„Abnehmen“ nur bei Human Interaction; ohne eigenes Repo wird das Ticket nur erledigt', async () => {
    const actor = backend.actor();
    await backend.services.projects.create('ARC', 'Arches', actor);
    const own = await backend.services.boards.create('ARC', 'Sprint', actor);
    const t = await backend.services.tickets.create(own.id, { title: 'Ansehen', acceptance: 'ok', needsPlanning: false }, actor);
    const refused = await server.post(`/api/dispatch/accept/${t.id}`);
    expect(refused.status).toBe(422);
    expect(refused.json.code).toBe('accept-not-possible');
    await backend.services.tickets.update(t.id, { status: 'blocked', blockedReason: 'Human Interaction: Oberfläche ansehen', branch: 'ticket/x' }, actor, { force: true });
    const accepted = await server.post(`/api/dispatch/accept/${t.id}`, { by: 'Florian' });
    expect(accepted.status).toBe(200);
    expect(accepted.json.result).toEqual({ outcome: 'accepted', text: expect.stringContaining(`${t.id} abgenommen und erledigt`) });
    expect(await backend.services.tickets.get(t.id)).toMatchObject({ status: 'done', blockedReason: null, branch: 'ticket/x' });
    expect((await server.post(`/api/dispatch/accept/${t.id}`, { by: 3 })).json.code).toBe('input-invalid');
  });

  it('Review und „Zurück an den Agenten“: Eingaben, Ablehnungen, fremdes Board und Erfolgsfall', async () => {
    const repo = new TempRepo();
    try {
      const actor = backend.actor();
      await backend.services.projects.create('ARC', 'Arches', actor);
      const own = await backend.services.boards.create('ARC', 'Sprint', actor);
      await backend.services.boards.setRepoPath(own.id, repo.path);
      await new BoardConfigStore(server.home).write({ ...MEMORY_CONFIG, activeBoard: own.id });
      repo.git('branch', 'ticket/x');
      const t = await backend.services.tickets.create(own.id, { title: 'Ansehen', acceptance: 'ok', needsPlanning: false }, actor);

      expect((await server.post(`/api/dispatch/return/${t.id}`, {})).json.code).toBe('input-invalid');
      expect((await server.post(`/api/dispatch/return/${t.id}`, { comment: 3 })).json.code).toBe('input-invalid');
      const none = await server.post(`/api/dispatch/return/${t.id}`, { comment: 'Bitte nachbessern' });
      expect(none.status).toBe(422);
      expect(none.json.code).toBe('return-not-possible');

      await backend.services.tickets.update(t.id, { status: 'blocked', blockedReason: 'Human Interaction: Review vor dem Merge', branch: 'ticket/x' }, actor, { force: true });
      const review = await server.get(`/api/dispatch/review/${t.id}`);
      expect(review.status).toBe(200);
      expect(review.json).toMatchObject({ ticketId: t.id, branch: 'ticket/x', available: true, files: [], tests: [], testsAt: null });
      const unknown = await server.get(`/api/dispatch/review/${t.id}/file?path=${encodeURIComponent('--output=x')}`);
      expect(unknown.status).toBe(422);
      expect(unknown.json.code).toBe('review-file-unknown');
      expect((await server.get(`/api/dispatch/review/${t.id}/file`)).json.code).toBe('input-invalid');

      const returned = await server.post(`/api/dispatch/return/${t.id}`, { comment: 'Bitte nachbessern', by: 'Florian' });
      expect(returned.status).toBe(200);
      expect(returned.json.result).toMatchObject({ outcome: 'returned' });
      expect(returned.json).toHaveProperty('enabled');
      expect(await backend.services.tickets.get(t.id)).toMatchObject({ status: 'planned', blockedReason: null, branch: 'ticket/x' });

      const other = { id: '22222222-2222-2222-2222-222222222222', name: 'Kollege' };
      await backend.db.transaction((u) => u.users.upsert(other));
      const foreign = await backend.services.boards.create('ARC', 'Fremd', other);
      const theirs = await backend.services.tickets.create(foreign.id, { title: 'Fremdes', acceptance: 'ok', needsPlanning: false }, other);
      await backend.services.tickets.update(theirs.id, { status: 'blocked', blockedReason: 'Human Interaction: Review vor dem Merge', branch: 'ticket/y' }, other, { force: true });
      expect((await server.get(`/api/dispatch/review/${theirs.id}`)).json).toMatchObject({ available: false, reason: expect.stringContaining('anderen Benutzer') });
      expect((await server.post(`/api/dispatch/return/${theirs.id}`, { comment: 'x' })).json.code).toBe('return-not-possible');
    } finally {
      repo.dispose();
    }
  });

  it('„Planen“ startet sofort, liefert Lauf und Zustand und lehnt Doppelstart und Boards ohne Repo ab', async () => {
    const repo = new TempRepo();
    try {
      const actor = backend.actor();
      await backend.services.projects.create('ARC', 'Arches', actor);
      const board = await backend.services.boards.create('ARC', 'Sprint', actor);
      const t = await backend.services.tickets.create(board.id, { title: 'Planbar', needsPlanning: true }, actor);
      const refused = await server.post(`/api/dispatch/plan/${t.id}`, { boardId: board.id });
      expect(refused.status).toBe(422);
      expect(refused.json.code).toBe('plan-no-repo');
      expect((await server.post('/api/dispatch/plan/ARC-0099', { boardId: board.id })).status).toBe(404);

      await backend.services.boards.setRepoPath(board.id, repo.path);
      await new BoardConfigStore(server.home).write({ ...MEMORY_CONFIG, activeBoard: board.id });
      expect((await server.get(`/api/dispatch/plan/${t.id}`)).json).toMatchObject({ ticketId: t.id, run: null, pausedUntil: null });
      const started = await server.post(`/api/dispatch/plan/${t.id}`, { boardId: board.id });
      expect(started.status).toBe(200);
      expect(started.json.started).toMatchObject({ kind: 'plan', ticketId: t.id });
      expect(started.json).toMatchObject({ enabled: false, active: 0, planning: 1 });
      expect((await server.get(`/api/dispatch/plan/${t.id.toLowerCase()}`)).json).toMatchObject({ run: { ticketId: t.id } });
      const again = await server.post(`/api/dispatch/plan/${t.id}`, { boardId: board.id });
      expect(again.json.code).toBe('plan-running');
    } finally {
      repo.dispose();
    }
  });

  it('„Recherche starten“ startet sofort, liefert Lauf und Zustand und lehnt Doppelstart, fremde und fehlende Fragen ab', async () => {
    const repo = new TempRepo();
    try {
      const actor = backend.actor();
      await backend.services.projects.create('ARC', 'Arches', actor);
      const board = await backend.services.boards.create('ARC', 'Sprint', actor);
      const question = { ...actor, kind: 'human' as const, session: null };
      const noRepo = await backend.services.questions.create('ARC', { title: 'Ohne Repo' }, question);
      const refused = await server.post(`/api/dispatch/research/${noRepo.id}`);
      expect(refused.status).toBe(422);
      expect(refused.json.code).toBe('research-no-repo');
      expect((await server.post('/api/dispatch/research/ARC-F0099')).status).toBe(404);
      expect((await server.post('/api/dispatch/research/kaputt')).json.code).toBe('question-id-invalid');
      const invalid = await server.get('/api/dispatch/research/kaputt');
      expect(invalid.status).toBe(422);
      expect(invalid.json).toMatchObject({ code: 'question-id-invalid', message: expect.stringContaining('„kaputt“ ist keine Fragenummer') });

      await backend.services.boards.setRepoPath(board.id, repo.path);
      await new BoardConfigStore(server.home).write({ ...MEMORY_CONFIG, activeBoard: board.id });
      expect((await server.get(`/api/dispatch/research/${noRepo.id}`)).json).toMatchObject({ questionId: noRepo.id, run: null, count: 0 });
      const started = await server.post(`/api/dispatch/research/${noRepo.id}`);
      expect(started.status).toBe(200);
      expect(started.json.started).toMatchObject({ kind: 'research', questionId: noRepo.id });
      expect(started.json).toMatchObject({ enabled: false, research: { active: 1 } });
      expect((await server.get(`/api/dispatch/research/${noRepo.id.toLowerCase()}`)).json).toMatchObject({ run: { questionId: noRepo.id }, count: 1 });
      const again = await server.post(`/api/dispatch/research/${noRepo.id}`);
      expect(again.status).toBe(422);
      expect(again.json.code).toBe('research-running');
    } finally {
      repo.dispose();
    }
  });

  it('listet und liest Protokolle, nur mit erlaubten Namen', async () => {
    mkdirSync(server.home.dispatchLogsDir, { recursive: true });
    writeFileSync(join(server.home.dispatchLogsDir, 'ARC-0001-20261015-090000.log'), 'Ausgabe');
    expect((await server.get('/api/dispatch/logs')).json).toEqual(['ARC-0001-20261015-090000.log']);
    const log = await server.get('/api/dispatch/logs/ARC-0001-20261015-090000.log');
    expect(log.text).toBe('Ausgabe');
    expect(String(log.headers['content-type'])).toContain('text/plain');
    expect((await server.get('/api/dispatch/logs/..%2Fboard.yaml')).status).toBe(422);
    expect((await server.get('/api/dispatch/logs/fehlt.log')).status).toBe(404);
  });

  it('listet die Läufe eines Tickets bzw. einer Frage, neueste zuerst, mit „läuft“ für aktive Läufe', async () => {
    mkdirSync(server.home.dispatchLogsDir, { recursive: true });
    for (const name of ['ARC-0001-20261015-090000.log', 'ARC-0001-20261016-101500-merge.log', 'ARC-00010-20261016-110000.log', 'ARC-F0002-20261016-120000-research.log']) {
      writeFileSync(join(server.home.dispatchLogsDir, name), 'x');
    }
    await new RunStore(server.home).create({
      id: 'ARC-0001-20261016-101500-merge', kind: 'merge', ticketId: 'ARC-0001', boardId: 1, title: 'T', agent: 'junior-ARC-0001', cwd: 'C:/w', branch: 'b',
      target: 'main', run: { profile: 'standard', why: '', model: null, effort: null, maxTurns: 10, timeoutMinutes: 10 }, command: 'claude', args: [], prompt: 'p',
      logFile: join(server.home.dispatchLogsDir, 'ARC-0001-20261016-101500-merge.log'), startedAt: new Date().toISOString(), pid: 1,
    });
    const runs = await server.get('/api/dispatch/runs?ticket=ARC-0001');
    expect(runs.status).toBe(200);
    expect(runs.json).toEqual([
      { name: 'ARC-0001-20261016-101500-merge.log', kind: 'merge', started: new Date(2026, 9, 16, 10, 15, 0).toISOString(), running: true },
      { name: 'ARC-0001-20261015-090000.log', kind: 'work', started: new Date(2026, 9, 15, 9, 0, 0).toISOString(), running: false },
    ]);
    expect((await server.get('/api/dispatch/runs?ticket=ARC-F0002')).json).toEqual([
      { name: 'ARC-F0002-20261016-120000-research.log', kind: 'research', started: new Date(2026, 9, 16, 12, 0, 0).toISOString(), running: false },
    ]);
    expect((await server.get('/api/dispatch/runs?ticket=ARC-0099')).json).toEqual([]);
    expect((await server.get('/api/dispatch/runs')).status).toBe(422);
    expect((await server.get('/api/dispatch/runs?ticket=..%2Fx')).json.code).toBe('id-invalid');
  });

  it('liefert ein Lauf-Protokoll zerlegt in Kopf, Arbeitsanweisung und Ausgabe', async () => {
    mkdirSync(server.home.dispatchLogsDir, { recursive: true });
    writeFileSync(join(server.home.dispatchLogsDir, 'ARC-0001-20261015-090000.log'),
      '[dispatch] Start\n[dispatch] Arbeitsanweisung:\nTu es.\n\n[dispatch] Ausgabe:\n09:00:01  Claude startet (opus)\n');
    const parts = await server.get('/api/dispatch/runs/ARC-0001-20261015-090000.log');
    expect(parts.json).toEqual({ name: 'ARC-0001-20261015-090000.log', header: '[dispatch] Start', prompt: 'Tu es.', body: '09:00:01  Claude startet (opus)\n' });
    expect((await server.get('/api/dispatch/runs/fehlt.log')).status).toBe(404);
    expect((await server.get('/api/dispatch/runs/..%2Fboard.yaml')).status).toBe(422);
  });

  it('liefert je Laufart Standardvorlage, Platzhalter und Pflichtteile', async () => {
    const info = (await server.get('/api/dispatch/config')).json.promptInfo;
    expect(info.map((p: any) => p.kind)).toEqual([...PROMPT_KINDS]);
    for (const entry of info) {
      expect(entry.label).toBeTruthy();
      expect(entry.default).toBe(DISPATCH_PROMPT_DEFAULTS[entry.kind as DispatchPromptKind]);
      expect(entry.placeholders.map((p: any) => p.name))
        .toEqual(DISPATCH_PROMPT_PLACEHOLDERS[entry.kind as DispatchPromptKind].map((p) => p.name));
      expect(entry.required.length).toBeGreaterThan(0);
    }
    expect(info.find((p: any) => p.kind === 'work').default).toContain('{{ticketId}}');
  });

  it('lehnt eine ungültige eigene Vorlage beim Speichern ab (kind, missing, unknown) und speichert nichts', async () => {
    const raw = (await server.get('/api/dispatch/config')).json.raw;
    const unknown = await server.put('/api/dispatch/config', { ...raw, prompts: { work: `${WORK}\n{{quatsch}}` } });
    expect(unknown.status).toBe(422);
    expect(unknown.json.code).toBe('dispatch-prompt-invalid');
    expect(unknown.json.details).toMatchObject({ kind: 'work', missing: [], unknown: ['quatsch'] });
    const missing = await server.put('/api/dispatch/config', { ...raw, prompts: { work: 'Nur {{agent}} und {{ticketId}}.' } });
    expect(missing.status).toBe(422);
    expect(missing.json.details.kind).toBe('work');
    expect(missing.json.details.missing.length).toBeGreaterThan(0);
    expect(missing.json.message).toContain('work');
    // nach beiden Fehlversuchen ist nichts geschrieben
    expect((await server.get('/api/dispatch/config')).json.exists).toBe(false);
    const ok = await server.put('/api/dispatch/config', { ...raw, prompts: { work: WORK } });
    expect(ok.status).toBe(200);
    expect(ok.json.raw.prompts.work).toBe(WORK);
  });

  it('zeigt eine Vorschau mit Beispielwerten, ohne etwas zu speichern', async () => {
    const preview = await server.post('/api/dispatch/prompts/preview', { kind: 'work' });
    expect(preview.status).toBe(200);
    expect(preview.json).toMatchObject({ kind: 'work', id: null, sample: true });
    expect(preview.json.prompt).toContain('AIH-0042');
    expect(preview.json.prompt).toContain('junior-AIH-0042');
    expect(preview.json.prompt).not.toMatch(/\{\{[A-Za-z]/);
    // Beispielantwort und Beispiel-Rückmeldung machen die optionalen Blöcke sichtbar
    expect(preview.json.prompt).toContain('Entscheidungen zu Fragen dieses Tickets');
    expect(preview.json.prompt).toContain('Rückmeldung aus dem Review');
  });

  it('rendert die Vorschau mit den echten Werten eines Tickets und einer Frage', async () => {
    const actor = backend.actor();
    await backend.services.projects.create('ARC', 'Arches', actor);
    const board = await backend.services.boards.create('ARC', 'Sprint', actor);
    const t = await backend.services.tickets.create(board.id, { title: 'Echtes Ticket', acceptance: 'ok', needsPlanning: false }, actor);
    const human: BoardQuestionActor = { userId: MEMORY_USER.id, name: 'Florian', kind: 'human', session: null };
    const q = await backend.services.questions.create('ARC', { title: 'Brutto oder netto?', ticketIds: [t.id] }, human);
    await backend.services.questions.answer(q.id, { text: 'Netto', reason: 'Vertrag' }, human);

    const work = (await server.post('/api/dispatch/prompts/preview', { kind: 'work', id: t.id })).json;
    expect(work).toMatchObject({ kind: 'work', id: t.id, sample: false });
    expect(work.prompt).toContain('Echtes Ticket');
    expect(work.prompt).toContain(t.id);
    expect(work.prompt).toContain('Netto');

    const research = (await server.post('/api/dispatch/prompts/preview', { kind: 'research', id: q.id })).json;
    expect(research).toMatchObject({ kind: 'research', id: q.id, sample: false });
    expect(research.prompt).toContain('Brutto oder netto?');

    expect((await server.post('/api/dispatch/prompts/preview', { kind: 'work', id: 'ARC-0999' })).status).toBe(404);
  });

  it('rendert in der Vorschau die eigene Vorlage, prüft sie und speichert nichts', async () => {
    const own = 'Plane {{ticketId}} („{{title}}“) als {{agent}}: harness board plan-json {{ticketId}} --file plan.json';
    const good = await server.post('/api/dispatch/prompts/preview', { kind: 'plan', template: own });
    expect(good.status).toBe(200);
    expect(good.json.prompt).toContain('Plane AIH-0042');
    const bad = await server.post('/api/dispatch/prompts/preview', { kind: 'plan', template: `${own}\n{{quatsch}}` });
    expect(bad.status).toBe(422);
    expect(bad.json.code).toBe('dispatch-prompt-invalid');
    expect(bad.json.details).toMatchObject({ kind: 'plan', unknown: ['quatsch'] });
    expect((await server.post('/api/dispatch/prompts/preview', { kind: 'quatsch' })).json.code).toBe('input-invalid');
    expect((await server.post('/api/dispatch/prompts/preview', { kind: 'plan', template: 7 })).json.code).toBe('input-invalid');
    // die Vorschau speichert nichts
    const view = (await server.get('/api/dispatch/config')).json;
    expect(view.exists).toBe(false);
    expect(view.config.prompts.plan).toBeNull();
  });
});
