// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ApiClient } from '../src/api/ApiClient.js';
import { BrowserPlatform } from '../src/platform/BrowserPlatform.js';
import { TauriPlatform } from '../src/platform/TauriPlatform.js';

// Das Benachrichtigungs-Plugin gibt es im Test nicht als Fenster der App; seine Funktionen sind gefälscht
const isPermissionGranted = vi.fn();
const requestPermission = vi.fn();
const sendNotification = vi.fn();
vi.mock('@tauri-apps/plugin-notification', () => ({
  isPermissionGranted: () => isPermissionGranted(),
  requestPermission: () => requestPermission(),
  sendNotification: (...args: unknown[]) => sendNotification(...args),
}));

// Browser-Meldung zum Mitschreiben; welche Erlaubnis gilt, setzt jeder Fall selbst
class FakeNotification {
  static permission = 'granted';
  static shown: FakeNotification[] = [];
  static asked: NotificationPermission = 'granted';
  static requestPermission = vi.fn(async () => FakeNotification.asked);
  onclick: ((event: { preventDefault: () => void }) => void) | null = null;
  closed = false;
  constructor(readonly title: string, readonly options: NotificationOptions) {
    FakeNotification.shown.push(this);
  }
  close() {
    this.closed = true;
  }
}

beforeEach(() => {
  FakeNotification.shown = [];
  FakeNotification.permission = 'granted';
  FakeNotification.asked = 'granted';
  vi.stubGlobal('Notification', FakeNotification);
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe('Meldungen der Desktop-App', () => {
  const platform = new TauriPlatform();

  it('kann auf jedem System dieser App Meldungen zeigen', () => {
    expect(platform.notificationsSupported()).toBe(true);
  });

  it('meldet die erteilte Erlaubnis des Systems', async () => {
    isPermissionGranted.mockResolvedValue(true);
    await expect(platform.notificationPermission()).resolves.toBe('granted');
  });

  it('gilt ohne Erlaubnis als noch nicht gefragt', async () => {
    isPermissionGranted.mockResolvedValue(false);
    await expect(platform.notificationPermission()).resolves.toBe('default');
  });

  it('fragt das System nicht noch einmal, wenn es die Erlaubnis schon gibt', async () => {
    isPermissionGranted.mockResolvedValue(true);
    await expect(platform.requestNotificationPermission()).resolves.toBe('granted');
    expect(requestPermission).not.toHaveBeenCalled();
  });

  it('holt die Erlaubnis beim System und gibt die Antwort weiter', async () => {
    isPermissionGranted.mockResolvedValue(false);
    requestPermission.mockResolvedValue('denied');
    await expect(platform.requestNotificationPermission()).resolves.toBe('denied');
    expect(requestPermission).toHaveBeenCalled();
  });

  it('zeigt die Meldung mit Titel und Text über das Plugin', async () => {
    await platform.notify({ title: 'Session braucht deine Antwort', body: 'webshop', tag: 'attention', url: '/p/p1/board/t/T-1' });
    expect(sendNotification).toHaveBeenCalledWith({ title: 'Session braucht deine Antwort', body: 'webshop' });
  });

  it('nimmt für die Meldung nicht die Browser-Meldung', async () => {
    await platform.notify({ title: 'Titel', body: 'Text', tag: 'tag', url: '/p/p1' });
    expect(FakeNotification.shown).toHaveLength(0);
  });

  it('macht aus einem Fehler des Plugins einen verständlichen Text', async () => {
    sendNotification.mockImplementationOnce(() => {
      throw new Error('not allowed');
    });
    await expect(platform.notify({ title: 'Titel', body: 'Text', tag: 'tag', url: '/p/p1' }))
      .rejects.toThrow('Die Meldung ließ sich nicht zeigen. (not allowed)');
  });
});

describe('Meldungen im Browser', () => {
  const platform = new BrowserPlatform({} as ApiClient);

  it('nimmt die Meldung des Browsers, wenn er sie kennt', () => {
    expect(platform.notificationsSupported()).toBe(true);
    vi.stubGlobal('Notification', undefined);
    expect(platform.notificationsSupported()).toBe(false);
  });

  it('meldet den Stand der Erlaubnis dieser Seite', async () => {
    FakeNotification.permission = 'default';
    await expect(platform.notificationPermission()).resolves.toBe('default');
  });

  it('gilt ohne Meldungen des Browsers als abgelehnt', async () => {
    vi.stubGlobal('Notification', undefined);
    await expect(platform.notificationPermission()).resolves.toBe('denied');
  });

  it('fragt die Erlaubnis beim Browser', async () => {
    FakeNotification.permission = 'default';
    FakeNotification.asked = 'granted';
    await expect(platform.requestNotificationPermission()).resolves.toBe('granted');
    expect(FakeNotification.requestPermission).toHaveBeenCalled();
  });

  it('zeigt die Meldung mit Text und Kennzeichen', async () => {
    await platform.notify({ title: 'Session ist bereit', body: 'webshop', tag: 's1', url: '/p/p1/sessions' });
    expect(FakeNotification.shown).toHaveLength(1);
    expect(FakeNotification.shown[0].title).toBe('Session ist bereit');
    expect(FakeNotification.shown[0].options).toMatchObject({ body: 'webshop', tag: 's1' });
  });

  it('holt beim Klick das Fenster nach vorn, ruft onClick und schließt die Meldung', async () => {
    const focus = vi.spyOn(window, 'focus').mockImplementation(() => undefined);
    const onClick = vi.fn();
    await platform.notify({ title: 'Titel', body: 'Text', tag: 'tag', url: '/p/p1/sessions', onClick });
    FakeNotification.shown[0].onclick?.({ preventDefault: () => undefined });
    expect(focus).toHaveBeenCalled();
    expect(onClick).toHaveBeenCalled();
    expect(FakeNotification.shown[0].closed).toBe(true);
    focus.mockRestore();
  });

  it('zeigt ohne Erlaubnis keine Meldung', async () => {
    FakeNotification.permission = 'denied';
    await platform.notify({ title: 'Titel', body: 'Text', tag: 'tag', url: '/p/p1' });
    expect(FakeNotification.shown).toHaveLength(0);
  });
});
