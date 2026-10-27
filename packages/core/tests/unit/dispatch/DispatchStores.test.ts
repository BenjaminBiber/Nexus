import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { HarnessHome } from '../../../src/registry/HarnessHome.js';
import { RunStore, type RunRecord } from '../../../src/dispatch/state/RunStore.js';
import { DispatchStateStore } from '../../../src/dispatch/state/DispatchStateStore.js';
import { DispatchLog } from '../../../src/dispatch/state/DispatchLog.js';
import { ReviewStore } from '../../../src/dispatch/state/ReviewStore.js';

const RUN = { profile: 'standard', why: 'x', model: 'opus', effort: 'high', maxTurns: 80, timeoutMinutes: 90 };

// Lauf-Datensatz mit Standardwerten
function record(id: string, startedAt: string, extra: Partial<RunRecord> = {}): RunRecord {
  return {
    id, kind: 'work', ticketId: 'ARC-0001', boardId: 1, title: 'T', agent: 'junior-ARC-0001', cwd: 'C:/wt', branch: 'ticket/x', target: null,
    run: RUN, command: 'claude', args: ['-p'], prompt: 'p', logFile: 'C:/log', startedAt, pid: null, ...extra,
  };
}

describe('Zustände des Dispatchers', () => {
  let dir: string;
  let home: HarnessHome;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'harness-dstore-'));
    home = new HarnessHome({ HARNESS_HOME: dir });
  });
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  it('RunStore legt Läufe an, merkt PID und Ergebnis und entfernt beides', async () => {
    const runs = new RunStore(home);
    await runs.create(record('b', '2026-10-15T10:01:00.000Z'));
    await runs.create(record('a', '2026-10-15T10:00:00.000Z'));
    await runs.setPid('a', 77);
    expect((await runs.list()).map((r) => [r.id, r.pid])).toEqual([['a', 77], ['b', null]]);
    expect(await runs.result('a')).toBeNull();
    await runs.finish('a', { code: 0, timedOut: false, output: 'ok', endedAt: '2026-10-15T10:05:00.000Z' });
    expect(await runs.result('a')).toMatchObject({ code: 0, output: 'ok' });
    expect((await runs.list()).map((r) => r.id)).toEqual(['a', 'b']);
    await runs.remove('a');
    expect(await runs.read('a')).toBeNull();
    expect(existsSync(join(dir, 'dispatch', 'runs', 'a.result.json'))).toBe(false);
  });

  it('DispatchStateStore schreibt Änderungen nacheinander', async () => {
    const state = new DispatchStateStore(home);
    expect(await state.read()).toEqual(DispatchStateStore.empty());
    await Promise.all([
      state.update((s) => { s.pausedUntil = '2026-10-15T11:00:00.000Z'; }),
      state.update((s) => { s.merges['ARC-0001'] = { ticketId: 'ARC-0001', branch: 'b', phase: 'pending', sessionUsed: false, waitNoted: false, target: null, since: 'x' }; }),
    ]);
    expect(await state.read()).toMatchObject({ pausedUntil: '2026-10-15T11:00:00.000Z', merges: { 'ARC-0001': { phase: 'pending' } } });
  });

  it('DispatchStateStore ergänzt den Zustand der Recherche- und Planungs-Automatik in älteren Dateien', async () => {
    mkdirSync(join(dir, 'dispatch'), { recursive: true });
    writeFileSync(home.dispatchStateFile, JSON.stringify({ pausedUntil: null, merges: {}, research: {}, researchAuto: { since: 'x' } }));
    const state = await new DispatchStateStore(home).read();
    expect(state.researchAuto).toEqual({ since: 'x', day: null, count: 0, retryAt: null, lastError: null });
    expect(state.planning).toEqual({});
    expect(state.planningAuto).toEqual({ since: null, retryAt: null, lastError: null });
  });

  it('DispatchLog schreibt Zeilen, benennt Lauf-Protokolle und liest nur erlaubte Namen', async () => {
    const log = new DispatchLog(home, () => new Date(2026, 9, 15, 9, 5, 7));
    log.line('gestartet ARC-0001');
    expect(readFileSync(join(dir, 'dispatch-logs', 'dispatch.log'), 'utf8')).toMatch(/^2026-10-15 09:05:07 {2}gestartet ARC-0001\n$/);
    const name = log.runName('ARC-0001', 'merge', new Date(2026, 9, 15, 9, 5, 7));
    expect(name).toBe('ARC-0001-20261015-090507-merge');
    writeFileSync(log.filePath(`${name}.log`), 'Inhalt');
    expect(await log.list()).toEqual(['dispatch.log', `${name}.log`]);
    expect(await log.read(`${name}.log`)).toBe('Inhalt');
    await expect(log.read('../board.yaml')).rejects.toMatchObject({ code: 'log-name-invalid' });
    await expect(log.read('fehlt.log')).rejects.toMatchObject({ code: 'log-not-found' });
  });

  it('DispatchLog listet die Läufe eines Tickets oder einer Frage, neueste zuerst, nur mit genau dieser ID', async () => {
    const log = new DispatchLog(home);
    expect(await log.runs('ARC-0001')).toEqual([]);
    mkdirSync(join(dir, 'dispatch-logs'), { recursive: true });
    for (const name of [
      'ARC-0001-20261015-090507.log', 'ARC-0001-20261016-101500-merge.log', 'ARC-0001-20261014-080000.log', 'ARC-00010-20261016-110000.log',
      'ARC-0001-x.log', 'dispatch.log', 'ARC-F0002-20261016-120000-research.log',
    ]) writeFileSync(log.filePath(name), 'x');
    expect(await log.runs('ARC-0001')).toEqual([
      { name: 'ARC-0001-20261016-101500-merge.log', kind: 'merge', started: new Date(2026, 9, 16, 10, 15, 0).toISOString() },
      { name: 'ARC-0001-20261015-090507.log', kind: 'work', started: new Date(2026, 9, 15, 9, 5, 7).toISOString() },
      { name: 'ARC-0001-20261014-080000.log', kind: 'work', started: new Date(2026, 9, 14, 8, 0, 0).toISOString() },
    ]);
    expect(await log.runs('arc-f0002')).toEqual([{ name: 'ARC-F0002-20261016-120000-research.log', kind: 'research', started: new Date(2026, 9, 16, 12, 0, 0).toISOString() }]);
    await expect(log.runs('../x')).rejects.toMatchObject({ code: 'id-invalid' });
  });

  it('RunStore: halb geschriebene Datei gilt als nicht vorhanden, andere Lesefehler werfen', async () => {
    const runs = new RunStore(home);
    mkdirSync(join(dir, 'dispatch', 'runs'), { recursive: true });
    writeFileSync(join(dir, 'dispatch', 'runs', 'halb.json'), '{"id": "ha');
    expect(await runs.read('halb')).toBeNull();
    expect(await runs.read('fehlt')).toBeNull();
    // Ein Ordner statt Datei steht für einen Lesefehler, der kein ENOENT ist (wie EBUSY/EPERM)
    mkdirSync(join(dir, 'dispatch', 'runs', 'ordner.json'));
    await expect(runs.read('ordner')).rejects.toMatchObject({ code: expect.stringMatching(/^E/) });
  });

  it('DispatchStateStore verschiebt kaputtes JSON beiseite und wirft bei anderen Lesefehlern', async () => {
    const state = new DispatchStateStore(home);
    mkdirSync(join(dir, 'dispatch'), { recursive: true });
    writeFileSync(join(dir, 'dispatch', 'state.json'), '{kaputt');
    expect(await state.read()).toEqual(DispatchStateStore.empty());
    const names = readdirSync(join(dir, 'dispatch'));
    expect(names.some((n) => n.startsWith('state.json.corrupt-'))).toBe(true);
    expect(names).not.toContain('state.json');
    await state.update((s) => { s.pausedUntil = 'x'; });
    expect((await state.read()).pausedUntil).toBe('x');
    rmSync(join(dir, 'dispatch', 'state.json'));
    mkdirSync(join(dir, 'dispatch', 'state.json'));
    await expect(state.read()).rejects.toMatchObject({ code: expect.stringMatching(/^E/) });
  });

  it('DispatchLog liest nur das Ende großer Dateien und nur Namen aus dem Ordner', async () => {
    const log = new DispatchLog(home);
    mkdirSync(join(dir, 'dispatch-logs'), { recursive: true });
    writeFileSync(log.filePath('gross.log'), `${'ä'.repeat(500_000)}${'a'.repeat(300_000)}ENDE`);
    const text = await log.read('gross.log');
    expect(text.length).toBe(200_001);
    expect(text.startsWith('…a')).toBe(true);
    expect(text.endsWith('ENDE')).toBe(true);
    await expect(log.read('CON.log')).rejects.toMatchObject({ code: 'log-not-found' });
  });
  it('ReviewStore speichert, liest und entfernt Review-Einträge; kaputtes JSON gilt als nicht vorhanden', async () => {
    const reviews = new ReviewStore(home);
    expect(await reviews.read('ARC-0001')).toBeNull();
    const entry = { ticketId: 'ARC-0001', branch: 'ticket/x', runId: 'r1', logFile: 'C:/log', at: '2026-10-19T10:00:00.000Z', tests: [{ command: 'npm test', ok: true, output: 'ok' }] };
    await reviews.save(entry);
    expect(await reviews.read('ARC-0001')).toEqual(entry);
    await reviews.save({ ...entry, runId: 'r2', tests: [] });
    expect((await reviews.read('ARC-0001'))?.runId).toBe('r2');
    await reviews.remove('ARC-0001');
    expect(await reviews.read('ARC-0001')).toBeNull();
    mkdirSync(home.dispatchReviewsDir, { recursive: true });
    writeFileSync(join(home.dispatchReviewsDir, 'ARC-0002.json'), '{ halb');
    expect(await reviews.read('ARC-0002')).toBeNull();
  });
});
