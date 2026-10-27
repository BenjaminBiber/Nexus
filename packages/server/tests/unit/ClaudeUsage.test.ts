import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ClaudeUsageService, type UsageFetch } from '../../src/usage/ClaudeUsageService.js';
import { TestServer } from '../helpers/TestServer.js';

const NOW = Date.parse('2026-10-20T10:00:00Z');
const BODY = {
  five_hour: { utilization: 79.4, resets_at: '2026-10-20T11:19:59+00:00' },
  seven_day: { utilization: 29, resets_at: '2026-10-22T13:59:59+00:00' },
};

describe('ClaudeUsageService', () => {
  let dir: string;
  let file: string;
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'harness-usage-'));
    file = join(dir, '.credentials.json');
  });
  afterEach(() => rmSync(dir, { recursive: true, force: true }));

  const login = (extra: Record<string, unknown> = {}) =>
    writeFileSync(file, JSON.stringify({ claudeAiOauth: { accessToken: 'tok-secret', expiresAt: NOW + 3_600_000, subscriptionType: 'team', ...extra } }));
  const reply = (status: number, body: unknown = BODY): ReturnType<UsageFetch> => Promise.resolve({ ok: status < 400, status, json: async () => body });
  const service = (fetch: UsageFetch, ttlMs?: number) => new ClaudeUsageService({ credentialsFile: file, fetch, clock: () => new Date(NOW), ttlMs });

  it('liest beide Fenster und schickt das Token nur als Bearer an den Endpunkt', async () => {
    login();
    const fetch = vi.fn<UsageFetch>(() => reply(200));
    const usage = await service(fetch).current();
    expect(usage).toEqual({
      available: true, reason: null, plan: 'team', fetchedAt: new Date(NOW).toISOString(),
      fiveHour: { percent: 79, resetsAt: '2026-10-20T11:19:59+00:00' },
      sevenDay: { percent: 29, resetsAt: '2026-10-22T13:59:59+00:00' },
    });
    expect(fetch.mock.calls[0]![0]).toBe('https://api.anthropic.com/api/oauth/usage');
    expect(fetch.mock.calls[0]![1].headers).toMatchObject({ Authorization: 'Bearer tok-secret', 'anthropic-beta': 'oauth-2025-04-20' });
    expect(JSON.stringify(usage)).not.toContain('tok-secret');
  });

  it('fragt innerhalb der Gültigkeit nur einmal und ruft gleichzeitige Anfragen zusammen', async () => {
    login();
    const fetch = vi.fn<UsageFetch>(() => reply(200));
    const s = service(fetch);
    await Promise.all([s.current(), s.current()]);
    await s.current();
    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it('meldet fehlende Anmeldung, abgelaufenes Token und Fehler des Endpunkts ohne Werte', async () => {
    const never = vi.fn<UsageFetch>(() => reply(200));
    expect(await service(never).current()).toMatchObject({ available: false, reason: 'no-login' });

    login({ expiresAt: NOW - 1 });
    expect(await service(never).current()).toMatchObject({ available: false, reason: 'token-expired', plan: 'team' });
    expect(never).not.toHaveBeenCalled();

    login();
    expect(await service(() => reply(401)).current()).toMatchObject({ available: false, reason: 'token-expired' });
    expect(await service(() => reply(500)).current()).toMatchObject({ available: false, reason: 'request-failed' });
    expect(await service(() => Promise.reject(new Error('offline'))).current()).toMatchObject({ available: false, reason: 'request-failed' });
  });

  it('merkt sich einen Fehlschlag nur kurz und fragt danach wieder an', async () => {
    login();
    let t = NOW;
    let status = 500;
    const fetch = vi.fn<UsageFetch>(() => reply(status));
    const s = new ClaudeUsageService({ credentialsFile: file, fetch, clock: () => new Date(t) });
    expect(await s.current()).toMatchObject({ available: false, reason: 'request-failed' });
    t += 5_000;
    status = 200;
    expect(await s.current()).toMatchObject({ available: true });
    expect(fetch).toHaveBeenCalledTimes(2);
  });

  it('stempelt den Zwischenspeicher erst nach dem Laden', async () => {
    login();
    let t = NOW;
    // Die Anfrage selbst braucht Zeit; die Gültigkeit darf erst nach ihrem Abschluss beginnen
    const fetch = vi.fn<UsageFetch>(() => {
      t += 50_000;
      return reply(200);
    });
    const s = new ClaudeUsageService({ credentialsFile: file, fetch, clock: () => new Date(t) });
    await s.current();
    t += 20_000; // 20 s nach dem Abschluss, also noch innerhalb der 60 s
    await s.current();
    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it('lässt ein fehlendes Fenster leer', async () => {
    login();
    const usage = await service(() => reply(200, { five_hour: { utilization: 3, resets_at: null }, seven_day: null })).current();
    expect(usage).toMatchObject({ available: true, fiveHour: { percent: 3, resetsAt: null }, sevenDay: null });
  });
});

describe('GET /api/claude-usage', () => {
  let server: TestServer;
  afterEach(() => server.dispose());
  const value = { available: true, reason: null, plan: 'max', fiveHour: { percent: 10, resetsAt: null }, sevenDay: null, fetchedAt: 'x' };

  it('bleibt ohne eingeschaltete Einstellung bei „disabled“ und fragt die Quelle nie', async () => {
    const current = vi.fn(async () => value);
    server = await TestServer.start({ claudeUsage: { current } });
    const res = await server.get('/api/claude-usage');
    expect(res.status).toBe(200);
    expect(res.json).toEqual({ available: false, reason: 'disabled', plan: null, fiveHour: null, sevenDay: null, fetchedAt: null });
    expect(current).not.toHaveBeenCalled();
  });

  it('liefert nach dem Einschalten, was die Quelle meldet, und wieder „disabled“ nach dem Ausschalten', async () => {
    const current = vi.fn(async () => value);
    server = await TestServer.start({ claudeUsage: { current } });
    expect((await server.put('/api/user-settings', { claudeUsage: true })).json.claudeUsage).toBe(true);
    expect((await server.get('/api/claude-usage')).json).toEqual(value);
    await server.put('/api/user-settings', { claudeUsage: false });
    expect((await server.get('/api/claude-usage')).json.reason).toBe('disabled');
    expect(current).toHaveBeenCalledTimes(1);
  });

  it('lehnt einen Schalter ab, der kein Wahrheitswert ist', async () => {
    server = await TestServer.start();
    const res = await server.put('/api/user-settings', { claudeUsage: 'ja' });
    expect(res.status).toBe(422);
    expect(res.json.message).toBe('claudeUsage muss true oder false sein.');
  });
});
