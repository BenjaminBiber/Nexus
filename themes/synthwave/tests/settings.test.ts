// Eigene Einstellungen: dasselbe Schema in manifest.json und im Code, fehlende oder ungültige Werte ergeben den Default
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import theme from '../src/main.js';
import { SYNTH_SETTINGS, resolveSettings, rootAttributes } from '../src/settings.js';

const manifest = JSON.parse(readFileSync(join(import.meta.dirname, '..', 'manifest.json'), 'utf8'));

describe('Einstellungen „Synthwave“', () => {
  it('stehen gleich im Manifest und in der ThemeDefinition', () => {
    expect(manifest.settingsSchema).toEqual(SYNTH_SETTINGS);
    expect(theme.settingsSchema).toEqual(SYNTH_SETTINGS);
  });

  it('jeder Default einer Auswahl ist eine ihrer Optionen', () => {
    for (const setting of Object.values(SYNTH_SETTINGS)) {
      if (setting.type === 'select') expect(setting.options.map((o) => o.value)).toContain(setting.default);
    }
  });

  it('ergänzt fehlende Werte mit dem Default', () => {
    expect(resolveSettings({})).toEqual({
      palette: 'outrun',
      glow: 'normal',
      horizon: 'animated',
      sky: 'auto',
      vhs: false,
      boot: true,
    });
  });

  it('verwirft ungültige Werte', () => {
    const resolved = resolveSettings({ palette: 'rosa', glow: 3, vhs: 'ja', boot: 0, sky: 'night' });
    expect(resolved.palette).toBe('outrun');
    expect(resolved.glow).toBe('normal');
    expect(resolved.vhs).toBe(false);
    expect(resolved.boot).toBe(true);
    expect(resolved.sky).toBe('night');
  });

  it('übersetzt die Standardwerte in data-synth-*-Attribute', () => {
    expect(rootAttributes(resolveSettings({}))).toEqual({
      'data-synth-palette': 'outrun',
      'data-synth-glow': 'normal',
      'data-synth-horizon': 'animated',
      'data-synth-vhs': 'off',
    });
  });

  it('übersetzt abweichende Werte in data-synth-*-Attribute', () => {
    expect(rootAttributes(resolveSettings({ palette: 'miami', glow: 'high', horizon: 'off', vhs: true }))).toEqual({
      'data-synth-palette': 'miami',
      'data-synth-glow': 'high',
      'data-synth-horizon': 'off',
      'data-synth-vhs': 'on',
    });
  });
});
