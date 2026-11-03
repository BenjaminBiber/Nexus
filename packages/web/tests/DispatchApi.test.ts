import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiClient } from '../src/api/ApiClient.js';
import { checkForm, toForm, toRaw } from '../src/components/dispatch/dispatchForm.js';
import type { RawDispatchConfigDto } from '../src/api/dispatchTypes.js';

afterEach(() => vi.unstubAllGlobals());

describe('ApiClient für den Dispatcher', () => {
  it('spricht die Routen unter /api/dispatch an', async () => {
    const calls: string[] = [];
    vi.stubGlobal('fetch', vi.fn(async (url: string, init: RequestInit) => {
      calls.push(`${init.method ?? 'GET'} ${url} ${init.body ?? ''}`);
      return new Response(url.includes('/logs/') ? 'Text' : '{}', { status: 200 });
    }));
    const api = new ApiClient();
    await api.getDispatchConfig();
    await api.saveDispatchConfig({ wip: 2 });
    await api.getDispatchStatus(3);
    await api.getDispatchStatus();
    await api.runDispatch(3);
    await api.pauseDispatch(15, 3);
    await api.resumeDispatch(3);
    await api.setDispatchEnabled(true, 3);
    await api.setDispatchEnabled(false);
    await api.retryDispatchMerge('ARC-0001', 3);
    await api.acceptDispatchTicket('ARC-0002', 3);
    await api.getDispatchReview('ARC-0002');
    await api.getDispatchReviewFile('ARC-0002', 'src/a b.ts');
    await api.returnDispatchTicket('ARC-0002', 'Bitte nachbessern', 3);
    await api.getQuestionResearch('ARC-F0007');
    await api.startQuestionResearch('ARC-F0007');
    await api.getTicketPlanning('ARC-0003');
    await api.startTicketPlanning('ARC-0003', 3);
    await api.getDispatchLogs();
    expect(await api.getDispatchLog('ARC-0001-x.log')).toBe('Text');
    await api.getDispatchRuns('ARC-0001');
    await api.getDispatchRunLog('ARC-0001-x.log');
    expect(calls).toEqual([
      'GET /api/dispatch/config ',
      'PUT /api/dispatch/config {"wip":2}',
      'GET /api/dispatch/status?boardId=3 ',
      'GET /api/dispatch/status ',
      'POST /api/dispatch/run {"boardId":3}',
      'POST /api/dispatch/pause {"minutes":15,"boardId":3}',
      'POST /api/dispatch/resume {"boardId":3}',
      'POST /api/dispatch/enable {"boardId":3}',
      'POST /api/dispatch/disable {}',
      'POST /api/dispatch/merge/ARC-0001 {"boardId":3}',
      'POST /api/dispatch/accept/ARC-0002 {"boardId":3}',
      'GET /api/dispatch/review/ARC-0002 ',
      'GET /api/dispatch/review/ARC-0002/file?path=src%2Fa%20b.ts ',
      'POST /api/dispatch/return/ARC-0002 {"comment":"Bitte nachbessern","boardId":3}',
      'GET /api/dispatch/research/ARC-F0007 ',
      'POST /api/dispatch/research/ARC-F0007 {}',
      'GET /api/dispatch/plan/ARC-0003 ',
      'POST /api/dispatch/plan/ARC-0003 {"boardId":3}',
      'GET /api/dispatch/logs ',
      'GET /api/dispatch/logs/ARC-0001-x.log ',
      'GET /api/dispatch/runs?ticket=ARC-0001 ',
      'GET /api/dispatch/runs/ARC-0001-x.log ',
    ]);
  });
});

describe('dispatchForm', () => {
  it('übersetzt Einstellung in Formular und zurück, ohne Unbekanntes zu verlieren', () => {
    const raw = {
      enabled: true, boards: [2], wip: 1, window: { from: '18:00', to: '07:00' }, optInLabel: 'auto', optOutLabel: 'manuell', maxAttempts: 2,
      pauseMinutesOnLimit: 60, promptExtra: 'x', agentPrefix: 'junior-',
      profiles: { klein: { model: 'sonnet', effort: 'high', maxTurns: 40, timeoutMinutes: 45 } }, defaultProfile: 'klein',
      rules: [{ when: { labels: ['doku'], filesMin: 6 }, profile: 'klein' }], escalate: true, allowFable: false,
      claude: { command: 'claude', permissionMode: 'acceptEdits', allowedTools: ['Read'], useSubscription: true, model: 'sonnet' },
    };
    const form = toForm(raw);
    expect(form).toMatchObject({ wip: '1', from: '18:00', to: '07:00', allowedTools: 'Read' });
    expect(form.gitBashPath).toBe('');
    expect(toRaw({ ...form, gitBashPath: ' C:/G/bin/bash.exe ' }, raw).claude).toMatchObject({ gitBashPath: 'C:/G/bin/bash.exe' });
    const withBash = { ...raw, claude: { ...raw.claude, gitBashPath: 'D:/bash.exe' } };
    expect(toForm(withBash).gitBashPath).toBe('D:/bash.exe');
    expect(toRaw(toForm(withBash), withBash)).toEqual(withBash);
    expect(toRaw({ ...toForm(withBash), gitBashPath: '' }, withBash).claude).toMatchObject({ gitBashPath: null });
    expect(form.rules[0]).toEqual({ labels: 'doku', priority: '', filesMin: '6', planMinChars: '', profile: 'klein', accounts: '' });
    expect(toRaw(form, raw)).toEqual(raw);
    const cleared = toRaw({ ...form, from: '', to: '', optIn: '' }, raw);
    expect(cleared).toMatchObject({ window: null, optInLabel: null });
  });

  it('Merge-Strategie: Vorgabe merge, ohne Änderung kein neues Feld, squash und zurück bleibt erhalten', () => {
    const raw = {};
    const form = toForm(raw);
    expect(form.mergeStrategy).toBe('merge');
    expect(toRaw(form, raw)).not.toHaveProperty('mergeStrategy');
    expect(toRaw({ ...form, mergeStrategy: 'squash' }, raw)).toMatchObject({ mergeStrategy: 'squash' });
    const squash = { mergeStrategy: 'squash' as const };
    expect(toForm(squash).mergeStrategy).toBe('squash');
    expect(toRaw({ ...toForm(squash), mergeStrategy: 'merge' }, squash)).toMatchObject({ mergeStrategy: 'merge' });
  });

  it('Einrichtung: ohne Befehle kein neues Feld, leere Zeilen fallen weg, leerer Ordner = Worktree', () => {
    const raw = {};
    const form = toForm(raw);
    expect(form.worktreeSetup).toEqual([]);
    expect(toRaw(form, raw)).not.toHaveProperty('worktreeSetup');
    const rows = [{ cwd: ' BackofficeNextGen ', run: ' npm ci ' }, { cwd: '', run: '' }, { cwd: '', run: 'npm run build' }];
    expect(toRaw({ ...form, worktreeSetup: rows }, raw).worktreeSetup).toEqual([{ cwd: 'BackofficeNextGen', run: 'npm ci' }, { cwd: '.', run: 'npm run build' }]);
    const saved = { worktreeSetup: [{ cwd: 'a', run: 'x' }] };
    expect(toForm(saved).worktreeSetup).toEqual([{ cwd: 'a', run: 'x' }]);
    expect(toRaw({ ...toForm(saved), worktreeSetup: [] }, saved)).toMatchObject({ worktreeSetup: [] });
  });

  it('Worktrees: Vorgabe ein Worktree je Ticket, ohne Änderung kein neues Feld, sameSpec und zurück bleibt erhalten', () => {
    const raw = {};
    const form = toForm(raw);
    expect(form.joinWorktrees).toBe('none');
    expect(toRaw(form, raw)).not.toHaveProperty('joinWorktrees');
    expect(toRaw({ ...form, joinWorktrees: 'sameSpec' }, raw)).toMatchObject({ joinWorktrees: 'sameSpec' });
    const bundled = { joinWorktrees: 'sameSpec' as const };
    expect(toForm(bundled).joinWorktrees).toBe('sameSpec');
    expect(toRaw({ ...toForm(bundled), joinWorktrees: 'none' }, bundled)).toMatchObject({ joinWorktrees: 'none' });
  });
});

describe('dispatchForm Recherchen', () => {
  it('Rundweg: research bleibt erhalten, ohne Abschnitt und ohne Änderung kommt keiner dazu', () => {
    const raw = { profiles: { standard: {} }, research: { auto: true, wip: 2, profile: 'standard', extra: 'x' } } as never;
    const form = toForm(raw);
    expect(form).toMatchObject({ researchAuto: true, researchWip: '2', researchProfile: 'standard' });
    expect(toRaw(form, raw).research).toEqual({ auto: true, wip: 2, profile: 'standard', extra: 'x' });
    const plain = { profiles: { standard: {} } };
    const empty = toForm(plain);
    expect(empty).toMatchObject({ researchAuto: false, researchWip: '1', researchProfile: '' });
    expect(toRaw(empty, plain)).not.toHaveProperty('research');
    expect(toRaw({ ...empty, researchAuto: true }, plain).research).toEqual({ auto: true, wip: 1, profile: null });
    expect(checkForm({ ...empty, researchWip: '0' })).toBe('Gleichzeitige Recherchen muss eine ganze Zahl von 1 bis 5 sein.');
    expect(checkForm({ ...empty, researchWip: '6' })).toBe('Gleichzeitige Recherchen muss eine ganze Zahl von 1 bis 5 sein.');
    expect(checkForm({ ...empty, researchMaxPerDay: '0' })).toBe('Automatische Recherchen je Tag muss eine ganze Zahl sein.');
    expect(toRaw({ ...empty, researchMaxPerDay: '3' }, plain).research).toEqual({ auto: false, wip: 1, maxPerDay: 3, profile: null });
    const withPerDay = { profiles: { standard: {} }, research: { auto: false, wip: 1, maxPerDay: 10 } };
    expect(toRaw(toForm(withPerDay), withPerDay).research).toEqual({ auto: false, wip: 1, maxPerDay: 10, profile: null });
  });
});

describe('dispatchForm Prüfung', () => {
  it('prüft Zahlen und doppelte Namen, erhält Unbekanntes in Profil und Regel', () => {
    const raw = { profiles: { a: { model: 'x', extra: 1 } }, rules: [{ when: { labels: ['l'] }, profile: 'a', note: 'n' }] } as never;
    const form = toForm(raw);
    expect(checkForm(form)).toBeNull();
    expect(checkForm({ ...form, wip: '1.5' })).toBe('Gleichzeitig (WIP) muss eine ganze Zahl sein.');
    expect(checkForm({ ...form, wip: '', maxAttempts: '0' })).toContain('Versuche');
    expect(checkForm({ ...form, profiles: [...form.profiles, { ...form.profiles[0], name: ' a' }] })).toBe('Profilname a doppelt');
    const out = toRaw(form, raw) as { profiles: Record<string, object>; rules: object[] };
    expect(out.profiles.a).toMatchObject({ extra: 1 });
    expect(out.rules[0]).toMatchObject({ note: 'n' });
  });
});

describe('dispatchForm Planungen', () => {
  it('Rundweg: planning bleibt erhalten, ohne Abschnitt und ohne Abweichung kommt keiner dazu, umbenannte Profile ziehen mit', () => {
    const raw = { profiles: { standard: {} }, planning: { profile: 'standard', extra: 'x' } } as never;
    const form = toForm(raw);
    expect(form).toMatchObject({ planningProfile: 'standard', planningAuto: false, planningWip: '1', planningAcceptance: true });
    expect(toRaw(form, raw).planning).toEqual({ profile: 'standard', extra: 'x', auto: false, wip: 1 });
    expect(toRaw({ ...form, planningProfile: '' }, raw).planning).toEqual({ profile: null, extra: 'x', auto: false, wip: 1 });
    const plain = { profiles: { standard: {} } };
    const empty = toForm(plain);
    expect(empty.planningProfile).toBe('');
    expect(toRaw(empty, plain)).not.toHaveProperty('planning');
    expect(toRaw({ ...empty, planningProfile: 'standard' }, plain).planning).toEqual({ profile: 'standard', auto: false, wip: 1 });
    // Automatik und Grenzen landen in der Datei, acceptance nur bei Abweichung von der Vorgabe (an) oder wenn es schon dastand
    expect(toRaw({ ...empty, planningAuto: true, planningWip: '3' }, plain).planning).toEqual({ profile: null, auto: true, wip: 3 });
    expect(toRaw({ ...empty, planningAcceptance: false }, plain).planning).toEqual({ profile: null, auto: false, wip: 1, acceptance: false });
    const off = { profiles: { standard: {} }, planning: { acceptance: false } };
    expect(toForm(off).planningAcceptance).toBe(false);
    expect(toRaw({ ...toForm(off), planningAcceptance: true }, off).planning).toEqual({ profile: null, auto: false, wip: 1, acceptance: true });
    // Ein altes Tageslimit fällt beim Speichern weg
    const old = { profiles: { standard: {} }, planning: { auto: true, maxPerDay: 4 } } as never;
    expect(toRaw(toForm(old), old).planning).toEqual({ profile: null, auto: true, wip: 1 });
  });
});

describe('dispatchForm – Accounts', () => {
  it('liest und schreibt Accounts je Profil und Regel; leere Felder entfernen die Liste', () => {
    const raw: RawDispatchConfigDto = {
      profiles: { schwer: { model: 'opus', accounts: ['max'] }, klein: { model: 'sonnet' } },
      defaultProfile: 'klein',
      rules: [{ when: { labels: ['firma'] }, profile: 'klein', accounts: ['pro-1', 'pro-2'] }],
      accountStrategy: 'order',
    };
    const form = toForm(raw);
    expect(form.profiles.map((p) => p.accounts)).toEqual(['max', '']);
    expect(form.rules[0].accounts).toBe('pro-1, pro-2');
    expect(form.accountStrategy).toBe('order');
    expect(toRaw(form, raw)).toMatchObject({ profiles: { schwer: { accounts: ['max'] } }, rules: [{ accounts: ['pro-1', 'pro-2'] }], accountStrategy: 'order' });
    const cleared = toRaw({ ...form, profiles: form.profiles.map((p) => ({ ...p, accounts: '' })), rules: form.rules.map((r) => ({ ...r, accounts: '' })) }, raw);
    expect(cleared.profiles!.schwer).not.toHaveProperty('accounts');
    expect(cleared.rules![0]).not.toHaveProperty('accounts');
    expect(toRaw(toForm({}), {})).not.toHaveProperty('accountLimitPercent');
    expect(checkForm({ ...form, accountLimit: '20' })).toBe('Account voll ab (%) muss eine ganze Zahl von 50 bis 100 sein.');
  });

  it('liest die Reserve mit Vorgaben 10/2/30 und prüft ihre Bereiche', () => {
    const form = toForm({});
    expect([form.accountReserveFiveHour, form.accountReserveWeek, form.accountResetGrace]).toEqual(['10', '2', '30']);
    expect(checkForm({ ...form, accountReserveFiveHour: '60' })).toBe('Reserve 5 Stunden (%) muss eine ganze Zahl von 0 bis 50 sein.');
    expect(checkForm({ ...form, accountReserveWeek: '51' })).toBe('Reserve Woche (%) muss eine ganze Zahl von 0 bis 50 sein.');
    expect(checkForm({ ...form, accountResetGrace: '301' })).toBe('Ohne Reserve, wenn 5 h zurücksetzt in (min) muss eine ganze Zahl von 0 bis 300 sein.');
    expect(checkForm({ ...form, accountReserveFiveHour: '0', accountReserveWeek: '50', accountResetGrace: '300' })).toBeNull();
  });

  it('schreibt die Reserve nur zurück, wenn sie abweicht oder schon in der Einstellung steht', () => {
    const plain = toRaw(toForm({}), {});
    expect(plain).not.toHaveProperty('accountReserveFiveHour');
    expect(plain).not.toHaveProperty('accountReserveWeek');
    expect(plain).not.toHaveProperty('accountResetGraceMinutes');
    const changed = toRaw({ ...toForm({}), accountReserveWeek: '5' }, {});
    expect(changed.accountReserveWeek).toBe(5);
    expect(changed).not.toHaveProperty('accountReserveFiveHour');
    expect(changed).not.toHaveProperty('accountResetGraceMinutes');
    const present = { accountReserveFiveHour: 10, accountResetGraceMinutes: 30 };
    expect(toForm(present).accountReserveFiveHour).toBe('10');
    expect(toRaw(toForm(present), present)).toMatchObject({ accountReserveFiveHour: 10, accountResetGraceMinutes: 30 });
    expect(toRaw({ ...toForm({}), accountResetGrace: '0' }, {}).accountResetGraceMinutes).toBe(0);
  });
});

