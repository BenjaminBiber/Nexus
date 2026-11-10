// Eigene Einstellungen des Synthwave-Themes; manifest.json hält dasselbe Schema (Test vergleicht beides).
// Die Shell spiegelt die Werte als data-synth-* auf <html>, damit theme.css auch Dialoge und Popovers außerhalb des Rahmens trifft.
import { useEffect, useMemo } from 'react';
import { useThemeSettings, type ThemeSettingsSchema } from '@nexus/theme-sdk';
import type { SkyPhase } from './sky.js';

export const SYNTH_SETTINGS = {
  palette: {
    type: 'select',
    label: 'Farbschema',
    options: [
      { value: 'outrun', label: 'Outrun' },
      { value: 'miami', label: 'Miami' },
      { value: 'nightcall', label: 'Nightcall' },
    ],
    default: 'outrun',
  },
  glow: {
    type: 'select',
    label: 'Leuchten',
    hint: 'Stärke des Neon-Leuchtens an Rahmen und Überschriften',
    options: [
      { value: 'normal', label: 'Normal' },
      { value: 'low', label: 'Dezent' },
      { value: 'high', label: 'Voll' },
    ],
    default: 'normal',
  },
  horizon: {
    type: 'select',
    label: 'Horizont',
    hint: 'Neon-Gitter am unteren Rand',
    options: [
      { value: 'animated', label: 'Bewegt' },
      { value: 'still', label: 'Still' },
      { value: 'off', label: 'Aus' },
    ],
    default: 'animated',
  },
  sky: {
    type: 'select',
    label: 'Himmel',
    options: [
      { value: 'auto', label: 'Nach Tageszeit' },
      { value: 'dawn', label: 'Morgenrot' },
      { value: 'day', label: 'Tag' },
      { value: 'dusk', label: 'Dämmerung' },
      { value: 'night', label: 'Nacht' },
    ],
    default: 'auto',
  },
  vhs: { type: 'boolean', label: 'VHS-Effekte', hint: 'Scanlines, dunkle Ränder und Bildstörung beim Seitenwechsel', default: false },
  boot: { type: 'boolean', label: 'Startanimation', hint: 'Sonnenaufgang über dem Gitter beim Laden der Oberfläche', default: true },
} as const satisfies ThemeSettingsSchema;

type Schema = typeof SYNTH_SETTINGS;

/** Gültige Werte aller Einstellungen */
export type SynthSettings = {
  [K in keyof Schema]: Schema[K] extends { type: 'select'; options: readonly { value: infer V }[] } ? V : boolean;
};

// Nimmt gespeicherte Werte nur, wenn sie zum Schema passen; sonst gilt der Default
export function resolveSettings(values: Record<string, unknown>): SynthSettings {
  const out: Record<string, boolean | string> = {};
  for (const [key, setting] of Object.entries(SYNTH_SETTINGS) as [string, Schema[keyof Schema]][]) {
    const value = values[key];
    if (setting.type === 'boolean') out[key] = typeof value === 'boolean' ? value : setting.default;
    else out[key] = typeof value === 'string' && setting.options.some((o) => o.value === value) ? value : setting.default;
  }
  return out as SynthSettings;
}

/** Einstellungen des Themes, immer vollständig (während des Ladens die Defaults) */
export function useSynthSettings(): SynthSettings & { loading: boolean } {
  const { values, loading } = useThemeSettings();
  return useMemo(() => ({ ...resolveSettings(values ?? {}), loading }), [values, loading]);
}

const onOff = (value: boolean) => (value ? 'on' : 'off');

/** Attribute für <html>, auf die theme.css reagiert; data-synth-sky ist die aufgelöste Phase des Himmels */
export function rootAttributes(settings: SynthSettings, sky: SkyPhase): Record<string, string> {
  return {
    'data-synth-palette': settings.palette,
    'data-synth-glow': settings.glow,
    'data-synth-horizon': settings.horizon,
    'data-synth-sky': sky,
    'data-synth-vhs': onOff(settings.vhs),
  };
}

/** Setzt die Attribute auf <html>, solange der Rahmen steht, und entfernt sie danach wieder */
export function useRootAttributes(settings: SynthSettings, sky: SkyPhase): void {
  const attributes = rootAttributes(settings, sky);
  const key = JSON.stringify(attributes);
  useEffect(() => {
    const root = document.documentElement;
    const entries = Object.entries(JSON.parse(key) as Record<string, string>);
    for (const [name, value] of entries) root.setAttribute(name, value);
    return () => {
      for (const [name] of entries) root.removeAttribute(name);
    };
  }, [key]);
}
