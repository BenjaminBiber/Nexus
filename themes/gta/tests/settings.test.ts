// @vitest-environment jsdom
// Eigene Einstellungen: fehlende oder ungültige Werte ergeben den Default; Stil und Ladebildschirm-Schalter überleben
// das Neuladen in localStorage, auch wenn der Speicher gesperrt ist
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  BOOT_KEY,
  GTA_SETTINGS,
  STYLE_KEY,
  rememberBoot,
  rememberStyle,
  rememberedBoot,
  rememberedStyle,
  resolveSettings,
  rootAttributes,
} from '../src/settings.js';

afterEach(() => {
  vi.restoreAllMocks();
  window.localStorage.clear();
});

describe('Einstellungen „GTA“', () => {
  it('jeder Default einer Auswahl ist eine ihrer Optionen', () => {
    for (const setting of Object.values(GTA_SETTINGS)) {
      if (setting.type === 'select') expect(setting.options.map((o) => o.value)).toContain(setting.default);
    }
  });

  it('ergänzt fehlende Werte mit dem Default', () => {
    expect(resolveSettings({})).toEqual({ style: 'v', accent: 'blau', radar: true, banners: true, boot: true, cheats: true });
  });

  it('verwirft ungültige Werte', () => {
    const resolved = resolveSettings({ style: 'iv', accent: 'rot', radar: 'aus', banners: 0, boot: false, cheats: false });
    expect(resolved).toEqual({ style: 'v', accent: 'blau', radar: true, banners: true, boot: false, cheats: false });
  });

  it('übersetzt die Standardwerte in data-gta-*-Attribute', () => {
    expect(rootAttributes(resolveSettings({}), 'v')).toEqual({
      'data-gta-style': 'v',
      'data-gta-accent': 'blau',
      'data-gta-radar': 'on',
    });
  });

  it('übersetzt abweichende Werte; der Stil kommt als eigener Wert', () => {
    expect(rootAttributes(resolveSettings({ style: 'v', accent: 'gruen', radar: false }), 'sa')).toEqual({
      'data-gta-style': 'sa',
      'data-gta-accent': 'gruen',
      'data-gta-radar': 'off',
    });
  });
});

describe('gemerkter Stil und Ladebildschirm', () => {
  it('ohne Eintrag gelten GTA V und Ladebildschirm an', () => {
    expect(rememberedStyle()).toBe('v');
    expect(rememberedBoot()).toBe(true);
  });

  it('merkt Stil und Schalter unter eigenen Schlüsseln', () => {
    rememberStyle('sa');
    rememberBoot(false);
    expect(window.localStorage.getItem(STYLE_KEY)).toBe('sa');
    expect(STYLE_KEY).toBe('nexus-gta-style');
    expect(BOOT_KEY).toBe('nexus-gta-boot');
    expect(rememberedStyle()).toBe('sa');
    expect(rememberedBoot()).toBe(false);
    rememberStyle('v');
    rememberBoot(true);
    expect(rememberedStyle()).toBe('v');
    expect(rememberedBoot()).toBe(true);
  });

  it('ein ungültiger gemerkter Stil ergibt GTA V', () => {
    window.localStorage.setItem(STYLE_KEY, 'vice-city');
    expect(rememberedStyle()).toBe('v');
  });

  it('schluckt Fehler des Speichers', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('gesperrt');
    });
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('gesperrt');
    });
    expect(() => rememberStyle('sa')).not.toThrow();
    expect(() => rememberBoot(false)).not.toThrow();
    expect(rememberedStyle()).toBe('v');
    expect(rememberedBoot()).toBe(true);
  });
});
