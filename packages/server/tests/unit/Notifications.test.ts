import { describe, expect, it } from 'vitest';
import type { SessionView, UserSettings, UserSettingsStore } from '@harness/core';
import { AttentionTracker } from '../../src/notifications/AttentionTracker.js';
import { BrowserNoticeHub, type BrowserNotice } from '../../src/notifications/BrowserNoticeHub.js';
import { NotificationService } from '../../src/notifications/NotificationService.js';
import { NtfySender } from '../../src/notifications/NtfySender.js';
import { ToastTexts } from '../../src/notifications/ToastTexts.js';
import { ToastXmlBuilder } from '../../src/notifications/ToastXmlBuilder.js';
import { WindowsToastSender } from '../../src/notifications/WindowsToastSender.js';

const view = (id: string, displayState: SessionView['displayState'], extra: Partial<SessionView> = {}) =>
  ({ id, projectId: 'p1', displayState, tickets: [], title: 'Favoriten als Seitenfenster', model: 'Sonnet 5.5', ...extra }) as unknown as SessionView;

describe('AttentionTracker', () => {
  it('meldet bereit nur nach läuft und wartet nur einmal', () => {
    const tracker = new AttentionTracker();
    expect(tracker.observe(undefined, view('a', 'idle'), 'A')).toBeNull();
    expect(tracker.observe('running', view('a', 'idle'), 'A')).toBe('bereit');
    expect(tracker.observe('running', view('a', 'waiting'), 'A')).toBe('wartet');
    expect(tracker.observe('waiting', view('a', 'waiting'), 'A')).toBeNull();
  });

  it('entfernt Sessions, die wieder laufen oder enden', () => {
    const tracker = new AttentionTracker();
    tracker.observe('running', view('a', 'idle'), 'A');
    tracker.observe('running', view('b', 'waiting'), 'B');
    tracker.observe('idle', view('a', 'running'), 'A');
    expect(tracker.list()).toEqual([{ kind: 'wartet', label: 'B' }]);
  });
});

describe('ToastTexts', () => {
  const texts = new ToastTexts();

  it('formuliert eine Einzelmeldung', () => {
    const model = texts.single({ kind: 'wartet', projectName: 'webshop', ticket: { id: 'T-0042', title: 'Favoriten' }, session: view('a', 'waiting'), openUrl: 'u' });
    expect(model).toMatchObject({ title: 'Session braucht deine Antwort', line1: 'webshop · T-0042 Favoriten', line2: 'Favoriten als Seitenfenster · Sonnet 5.5' });
  });

  it('zeigt ohne Ticket nur das Projekt', () => {
    const model = texts.single({ kind: 'bereit', projectName: 'webshop', session: view('a', 'idle'), openUrl: 'u' });
    expect(model).toMatchObject({ title: 'Session ist fertig', line1: 'webshop' });
  });

  it('fasst mehrere Sessions zusammen und kürzt die Liste', () => {
    const pending = [
      { kind: 'wartet' as const, label: 'T-1' },
      { kind: 'bereit' as const, label: 'T-2' },
      { kind: 'bereit' as const, label: 'T-3' },
      { kind: 'bereit' as const, label: 'T-4' },
      { kind: 'bereit' as const, label: 'T-5' },
    ];
    expect(texts.collected(pending, 'board')).toMatchObject({
      title: '5 Sessions warten auf dich',
      line1: '1 braucht eine Antwort, 4 sind fertig',
      line2: 'T-1 · T-2 · T-3 + 2 weitere',
    });
  });
});

describe('ToastXmlBuilder', () => {
  const xml = new ToastXmlBuilder();
  const base = { kind: 'wartet' as const, title: 'A & B', line1: 'x', line2: 'y', openUrl: 'http://127.0.0.1:1/p/p1/tickets/T-1' };
  const options = { assetsDir: 'C:\\assets', heroImages: false };

  it('maskiert Sonderzeichen und bietet kein Später erinnern an', () => {
    const out = xml.build(base, options);
    expect(out).toContain('<text>A &amp; B</text>');
    expect(out).not.toContain('scenario');
    expect(out).not.toContain('snooze');
    expect(out).not.toContain('Später erinnern');
    expect(out).not.toContain('placement="hero"');
  });

  it('zeigt das Hero nur bei wartet und eingeschalteter Einstellung', () => {
    expect(xml.build(base, { ...options, heroImages: true })).toContain('placement="hero"');
    expect(xml.build({ ...base, kind: 'bereit' }, { ...options, heroImages: true })).not.toContain('placement="hero"');
  });

  it('hat bei bereit nur Board öffnen und kein Ignorieren', () => {
    const out = xml.build({ ...base, kind: 'bereit' }, options);
    expect(out).not.toContain('scenario');
    expect(out).not.toContain('snooze');
    expect(out).toContain('Board öffnen');
    expect(out).not.toContain('gnorieren');
  });

  it('beschriftet die Schaltfläche mit Sessions öffnen, wenn sie die Sessions öffnet', () => {
    const out = xml.build({ ...base, openUrl: 'http://127.0.0.1:1/p/p1/sessions' }, options);
    expect(out).toContain('content="Sessions öffnen"');
    expect(out).not.toContain('Board öffnen');
  });
});

describe('NotificationService', () => {
  const store = (settings: Partial<UserSettings>) =>
    ({ read: async () => ({ notifications: 'windows', heroImages: false, ...settings }) }) as unknown as UserSettingsStore;

  const setup = (settings: Partial<UserSettings>) => {
    const shown: string[] = [];
    const sender = new WindowsToastSender(async (_script, env) => (shown.push(env.HARNESS_TOAST_XML), 0), 'win32');
    return { shown, service: new NotificationService(store(settings), sender, 'C:\\a', 'http://127.0.0.1:7777') };
  };
  const change = (v: SessionView, before: SessionView['displayState']) => ({ projectId: 'p1', projectName: 'webshop', before, view: v });

  it('zeigt im Modus windows einen Toast mit Adresse des Tickets', async () => {
    const { shown, service } = setup({});
    await service.onSessionChange({ ...change(view('a', 'waiting', { tickets: ['T-1'] }), 'running'), ticket: { id: 'T-1', title: 'Favoriten' } });
    expect(shown).toHaveLength(1);
    expect(shown[0]).toContain('http://127.0.0.1:7777/p/p1/board/t/T-1');
  });

  it('zeigt in anderen Modi nichts', async () => {
    const { shown, service } = setup({ notifications: 'browser' });
    await service.onSessionChange(change(view('a', 'waiting'), 'running'));
    expect(shown).toHaveLength(0);
  });

  it('fasst zwei wartende Sessions zu einer Meldung zusammen', async () => {
    const { shown, service } = setup({});
    await service.onSessionChange(change(view('a', 'waiting'), 'running'));
    await service.onSessionChange(change(view('b', 'idle'), 'running'));
    expect(shown[1]).toContain('2 Sessions warten auf dich');
  });

  it('verlinkt die zusammengefasste Meldung auf die Sessions des Projekts', async () => {
    const { shown, service } = setup({});
    await service.onSessionChange(change(view('a', 'waiting'), 'running'));
    await service.onSessionChange(change(view('b', 'idle'), 'running'));
    expect(shown[1]).toContain('http://127.0.0.1:7777/p/p1/sessions');
    expect(shown[1]).not.toContain('/board');
  });

  it('verlinkt eine Meldung ohne Ticket auf die Sessions des Projekts', async () => {
    const { shown, service } = setup({});
    await service.onSessionChange(change(view('a', 'idle'), 'running'));
    expect(shown[0]).toContain('http://127.0.0.1:7777/p/p1/sessions');
    expect(shown[0]).not.toContain('/board');
    expect(shown[0]).toContain('content="Sessions öffnen"');
  });

  it('zeigt als Testmeldung ein Beispiel mit einer Ticket-ID des DB-Boards', async () => {
    const { shown, service } = setup({});
    expect(await service.sendTest()).toBeNull();
    expect(shown[0]).toContain('ARC-0042 Favoriten');
    expect(shown[0]).not.toContain('T-0042');
  });
});

describe('NotificationService mit offenem Tab', () => {
  const store = { read: async () => ({ notifications: 'windows', heroImages: false }) } as unknown as UserSettingsStore;

  const setup = () => {
    const shown: string[] = [];
    const sender = new WindowsToastSender(async (_script, env) => (shown.push(env.HARNESS_TOAST_XML), 0), 'win32');
    const tabs = new BrowserNoticeHub();
    return { shown, tabs, service: new NotificationService(store, sender, 'C:\a', 'http://127.0.0.1:7777', () => undefined, tabs) };
  };
  const waiting = { projectId: 'p1', projectName: 'webshop', before: 'running' as const, view: view('a', 'waiting', { tickets: ['T-1'] }), ticket: { id: 'T-1', title: 'Favoriten' } };

  it('schickt die Meldung an den Tab statt an Windows, mit Pfad der Oberfläche', async () => {
    const { shown, tabs, service } = setup();
    const received: BrowserNotice[] = [];
    tabs.attach((notice) => received.push(notice));
    await service.onSessionChange(waiting);
    expect(shown).toHaveLength(0);
    expect(received).toEqual([
      expect.objectContaining({ type: 'notice', kind: 'wartet', tag: 'attention', title: 'Session braucht deine Antwort', url: '/p/p1/board/t/T-1' }),
    ]);
  });

  it('zeigt den Windows-Toast, sobald kein Tab mehr offen ist', async () => {
    const { shown, tabs, service } = setup();
    tabs.attach(() => undefined)();
    await service.onSessionChange(waiting);
    expect(shown).toHaveLength(1);
  });
});

describe('NtfySender', () => {
  const model = { kind: 'wartet' as const, title: 'Session braucht deine Antwort', line1: 'webshop · T-1', line2: 'Favoriten', openUrl: 'http://127.0.0.1:7777/p/p1' };
  const target = { server: 'https://ntfy.example/', topic: 'harness-x', token: '' };
  const capture = (status = 200) => {
    const calls: Array<{ url: string; headers: Record<string, string>; body: Record<string, unknown> }> = [];
    const sender = new NtfySender(async (url, init) => {
      calls.push({ url, headers: init.headers, body: JSON.parse(init.body) });
      return { ok: status < 300, status, text: async () => 'unauthorized' };
    });
    return { calls, sender };
  };

  it('veröffentlicht als JSON mit Thema, Text, Tag und hoher Priorität bei wartet', async () => {
    const { calls, sender } = capture();
    expect(await sender.send(target, model)).toBeNull();
    expect(calls[0].url).toBe('https://ntfy.example');
    expect(calls[0].headers.Authorization).toBeUndefined();
    expect(calls[0].body).toEqual({ topic: 'harness-x', title: 'Session braucht deine Antwort', message: 'webshop · T-1\nFavoriten', tags: ['raising_hand'], priority: 4 });
  });

  it('schickt das Token und verlinkt nur erreichbare Adressen', async () => {
    const { calls, sender } = capture();
    await sender.send({ ...target, token: 'tk_1' }, { ...model, kind: 'bereit', openUrl: 'http://harness.lan:7777/p/p1' });
    expect(calls[0].headers.Authorization).toBe('Bearer tk_1');
    expect(calls[0].body).toMatchObject({ priority: 3, click: 'http://harness.lan:7777/p/p1' });
  });

  it('meldet Fehler des Servers als Text', async () => {
    const { sender } = capture(401);
    expect(await sender.send(target, model)).toBe('ntfy antwortete mit 401: unauthorized.');
  });
});

describe('NotificationService mit ntfy', () => {
  const setup = (settings: Partial<UserSettings>) => {
    const shown: string[] = [];
    const pushed: string[] = [];
    const store = { read: async () => ({ notifications: 'off', heroImages: false, ntfy: true, ntfyServer: 'https://ntfy.sh', ntfyTopic: 'harness-x', ntfyToken: '', ...settings }) } as unknown as UserSettingsStore;
    const sender = new WindowsToastSender(async (_script, env) => (shown.push(env.HARNESS_TOAST_XML), 0), 'win32');
    const ntfy = new NtfySender(async (_url, init) => (pushed.push(init.body), { ok: true, status: 200, text: async () => '' }));
    return { shown, pushed, service: new NotificationService(store, sender, 'C:\a', 'http://127.0.0.1:7777', () => undefined, null, ntfy) };
  };
  const waiting = { projectId: 'p1', projectName: 'webshop', before: 'running' as const, view: view('a', 'waiting') };

  it('pusht auch im Modus aus, ohne Windows-Toast', async () => {
    const { shown, pushed, service } = setup({});
    await service.onSessionChange(waiting);
    expect(shown).toHaveLength(0);
    expect(pushed).toHaveLength(1);
    expect(JSON.parse(pushed[0]).title).toBe('Session braucht deine Antwort');
  });

  it('pusht im Modus windows zusätzlich zum Toast', async () => {
    const { shown, pushed, service } = setup({ notifications: 'windows' });
    await service.onSessionChange(waiting);
    expect(shown).toHaveLength(1);
    expect(pushed).toHaveLength(1);
  });

  it('pusht nicht ohne Thema oder ausgeschaltet', async () => {
    for (const settings of [{ ntfyTopic: '' }, { ntfy: false }]) {
      const { pushed, service } = setup(settings);
      await service.onSessionChange(waiting);
      expect(pushed).toHaveLength(0);
    }
  });

  it('meldet bei der Testmeldung ein fehlendes Thema', async () => {
    const { service } = setup({ ntfyTopic: '' });
    expect(await service.sendNtfyTest()).toBe('Für ntfy ist kein Thema eingetragen.');
  });
});
