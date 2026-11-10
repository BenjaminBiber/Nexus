// Eigene Einstellungen: dasselbe Schema in manifest.json und im Code, fehlende oder ungültige Werte ergeben den Default
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import theme from '../src/main.js';
import { TERMINAL_SETTINGS, resolveSettings, rootAttributes } from '../src/settings.js';

const manifest = JSON.parse(readFileSync(join(import.meta.dirname, '..', 'manifest.json'), 'utf8'));

describe('Einstellungen „Terminal“', () => {
  it('stehen gleich im Manifest und in der ThemeDefinition', () => {
    expect(manifest.settingsSchema).toEqual(TERMINAL_SETTINGS);
    expect(theme.settingsSchema).toEqual(TERMINAL_SETTINGS);
  });

  it('jeder Default einer Auswahl ist eine ihrer Optionen', () => {
    for (const setting of Object.values(TERMINAL_SETTINGS)) {
      if (setting.type === 'select') expect(setting.options.map((o) => o.value)).toContain(setting.default);
    }
  });

  it('ergänzt fehlende Werte mit dem Default', () => {
    expect(resolveSettings({})).toEqual({
      font: 'jetbrains',
      ligatures: false,
      fontSize: '13',
      icons: 'nerd',
      palette: 'github',
      crt: false,
      cursorBlink: true,
      typeOutput: false,
      boot: true,
    });
  });

  it('verwirft ungültige Werte', () => {
    const resolved = resolveSettings({ palette: 'rosa', crt: 'ja', font: 'plex' });
    expect(resolved.palette).toBe('github');
    expect(resolved.crt).toBe(false);
    expect(resolved.font).toBe('plex');
  });

  it('übersetzt die Werte in data-term-*-Attribute', () => {
    expect(rootAttributes(resolveSettings({ crt: true, ligatures: true, palette: 'nord', cursorBlink: false }))).toEqual({
      'data-term-font': 'jetbrains',
      'data-term-ligatures': 'on',
      'data-term-size': '13',
      'data-term-icons': 'nerd',
      'data-term-palette': 'nord',
      'data-term-crt': 'on',
      'data-term-blink': 'off',
    });
  });
});
