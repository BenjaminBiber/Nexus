import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { HarnessHome } from '../../../src/registry/HarnessHome.js';
import { UserSettingsStore } from '../../../src/settings/UserSettingsStore.js';

const DEFAULTS = { notifications: 'off', heroImages: false, theme: 'system', claudeUsage: false, compactCards: false, hideNotNeeded: false, hideInPlanning: false, ntfy: false, ntfyServer: 'https://ntfy.sh', ntfyTopic: '', ntfyToken: '' };

describe('UserSettingsStore', () => {
  let dir: string;
  let home: HarnessHome;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), 'harness-settings-'));
    home = new HarnessHome({ HARNESS_HOME: dir });
  });
  afterEach(() => rm(dir, { recursive: true, force: true }));

  it('liefert ohne Datei die Standardwerte', async () => {
    expect(await new UserSettingsStore(home).read()).toEqual(DEFAULTS);
  });

  it('speichert und liest die Einstellungen', async () => {
    const store = new UserSettingsStore(home);
    await store.write({ notifications: 'windows', heroImages: true, theme: 'dark', claudeUsage: true, compactCards: true, hideNotNeeded: true, hideInPlanning: true, ntfy: true, ntfyServer: 'https://push.example', ntfyTopic: 'harness-x', ntfyToken: 'tk_1' });
    expect(await store.read()).toEqual({ notifications: 'windows', heroImages: true, theme: 'dark', claudeUsage: true, compactCards: true, hideNotNeeded: true, hideInPlanning: true, ntfy: true, ntfyServer: 'https://push.example', ntfyTopic: 'harness-x', ntfyToken: 'tk_1' });
  });

  it('übernimmt den früheren Schalter als Browser-Meldung', async () => {
    await writeFile(home.settingsFile, JSON.stringify({ notifyOnWaiting: true }));
    expect((await new UserSettingsStore(home).read()).notifications).toBe('browser');
  });

  it('ersetzt ungültige Werte durch die Standardwerte', async () => {
    await writeFile(home.settingsFile, JSON.stringify({ notifications: 'laut', snoozeMinutes: 7 }));
    expect(await new UserSettingsStore(home).read()).toEqual(DEFAULTS);
  });

  it('fällt bei beschädigter Datei auf die Standardwerte zurück', async () => {
    await writeFile(home.settingsFile, '{kaputt');
    expect(await new UserSettingsStore(home).read()).toEqual(DEFAULTS);
  });

  it('ersetzt ein unbekanntes Theme durch system', async () => {
    await writeFile(home.settingsFile, JSON.stringify({ notifications: 'off', heroImages: false, theme: 'lila' }));
    expect((await new UserSettingsStore(home).read()).theme).toBe('system');
  });
});
