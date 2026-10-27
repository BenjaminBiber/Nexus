import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TestServer } from '../helpers/TestServer.js';

describe('UserSettingsRoutes', () => {
  let server: TestServer;

  beforeEach(async () => {
    server = await TestServer.start();
  });
  afterEach(() => server.dispose());

  it('liefert system als Standard und speichert das Theme', async () => {
    expect((await server.get('/api/user-settings')).json.theme).toBe('system');
    const saved = await server.put('/api/user-settings', { theme: 'dark' });
    expect(saved.status).toBe(200);
    expect(saved.json).toEqual({ notifications: 'off', heroImages: false, theme: 'dark', claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false, ntfy: false, ntfyServer: 'https://ntfy.sh', ntfyTopic: '', ntfyToken: '' });
    expect((await server.get('/api/user-settings')).json.theme).toBe('dark');
  });

  it('lehnt ein unbekanntes Theme ab', async () => {
    const res = await server.put('/api/user-settings', { theme: 'lila' });
    expect(res.status).toBe(422);
    expect(res.json.message).toBe('theme muss system, light, dark, signal, pastel oder win95 sein.');
  });

  it('speichert die kompakten Ticket-Kacheln', async () => {
    expect((await server.get('/api/user-settings')).json.compactCards).toBe(false);
    const saved = await server.put('/api/user-settings', { compactCards: true });
    expect(saved.status).toBe(200);
    expect(saved.json.compactCards).toBe(true);
    expect((await server.get('/api/user-settings')).json.compactCards).toBe(true);
  });

  it('lehnt compactCards ohne Wahrheitswert ab', async () => {
    const res = await server.put('/api/user-settings', { compactCards: 'ja' });
    expect(res.status).toBe(422);
    expect(res.json.message).toBe('compactCards muss true oder false sein.');
  });

  it('speichert die ntfy-Einstellungen', async () => {
    const saved = await server.put('/api/user-settings', { ntfy: true, ntfyServer: ' https://push.example ', ntfyTopic: 'harness-x', ntfyToken: 'tk_1' });
    expect(saved.status).toBe(200);
    expect(saved.json).toMatchObject({ ntfy: true, ntfyServer: 'https://push.example', ntfyTopic: 'harness-x', ntfyToken: 'tk_1' });
  });

  it('lehnt ein ntfy-Thema mit ungültigen Zeichen und einen Server ohne http ab', async () => {
    expect((await server.put('/api/user-settings', { ntfyTopic: 'mein thema' })).status).toBe(422);
    expect((await server.put('/api/user-settings', { ntfyServer: 'ntfy.sh' })).status).toBe(422);
  });
});
