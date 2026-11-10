// Eigene Einstellungen des Terminal-Themes; manifest.json hält dasselbe Schema (Test vergleicht beides).
// Die Shell spiegelt die Werte als data-term-* auf <html>, damit theme.css auch Dialoge und Popovers außerhalb des Rahmens trifft.
import { useEffect, useMemo } from 'react';
import { useThemeSettings, type ThemeSettingsSchema } from '@nexus/theme-sdk';

export const TERMINAL_SETTINGS = {
  font: {
    type: 'select',
    label: 'Schriftart',
    options: [
      { value: 'jetbrains', label: 'JetBrains Mono' },
      { value: 'plex', label: 'IBM Plex Mono' },
      { value: 'geist', label: 'Geist Mono' },
    ],
    default: 'jetbrains',
  },
  ligatures: { type: 'boolean', label: 'Ligaturen', hint: 'Zeichenfolgen wie => oder != als ein Zeichen (nur JetBrains Mono und Geist Mono)', default: false },
  fontSize: {
    type: 'select',
    label: 'Schriftgröße',
    options: [
      { value: '12', label: '12 px' },
      { value: '13', label: '13 px' },
      { value: '14', label: '14 px' },
    ],
    default: '13',
  },
  icons: {
    type: 'select',
    label: 'Symbole',
    hint: 'Symbole in Navigation und Docs-Baum',
    options: [
      { value: 'nerd', label: 'Nerd Font' },
      { value: 'unicode', label: 'Unicode' },
      { value: 'off', label: 'Keine' },
    ],
    default: 'nerd',
  },
  palette: {
    type: 'select',
    label: 'Farbschema',
    options: [
      { value: 'github', label: 'GitHub Dark' },
      { value: 'dracula', label: 'Dracula' },
      { value: 'gruvbox', label: 'Gruvbox' },
      { value: 'tokyonight', label: 'Tokyo Night' },
      { value: 'solarized', label: 'Solarized Dark' },
      { value: 'nord', label: 'Nord' },
      { value: 'green', label: 'Phosphor grün' },
      { value: 'amber', label: 'Phosphor bernstein' },
    ],
    default: 'github',
  },
  crt: { type: 'boolean', label: 'Röhrenmonitor', hint: 'Scanlines, leichtes Glühen und dunkle Ränder', default: false },
  cursorBlink: { type: 'boolean', label: 'Cursor blinkt', default: true },
  typeOutput: { type: 'boolean', label: 'Ausgabe tippen', hint: 'Ausgaben der Prompt-Zeile erscheinen Zeichen für Zeichen', default: false },
  boot: { type: 'boolean', label: 'Startanimation', hint: 'Boot-Meldungen beim Laden der Oberfläche', default: true },
} as const satisfies ThemeSettingsSchema;

type Schema = typeof TERMINAL_SETTINGS;

/** Gültige Werte aller Einstellungen */
export type TerminalSettings = {
  [K in keyof Schema]: Schema[K] extends { type: 'select'; options: readonly { value: infer V }[] } ? V : boolean;
};

// Nimmt gespeicherte Werte nur, wenn sie zum Schema passen; sonst gilt der Default
export function resolveSettings(values: Record<string, unknown>): TerminalSettings {
  const out: Record<string, boolean | string> = {};
  for (const [key, setting] of Object.entries(TERMINAL_SETTINGS) as [string, Schema[keyof Schema]][]) {
    const value = values[key];
    if (setting.type === 'boolean') out[key] = typeof value === 'boolean' ? value : setting.default;
    else out[key] = typeof value === 'string' && setting.options.some((o) => o.value === value) ? value : setting.default;
  }
  return out as TerminalSettings;
}

/** Einstellungen des Themes, immer vollständig (während des Ladens die Defaults) */
export function useTerminalSettings(): TerminalSettings & { loading: boolean } {
  const { values, loading } = useThemeSettings();
  return useMemo(() => ({ ...resolveSettings(values ?? {}), loading }), [values, loading]);
}

const onOff = (value: boolean) => (value ? 'on' : 'off');

/** Attribute für <html>, auf die theme.css reagiert */
export function rootAttributes(settings: TerminalSettings): Record<string, string> {
  return {
    'data-term-font': settings.font,
    'data-term-ligatures': onOff(settings.ligatures),
    'data-term-size': settings.fontSize,
    'data-term-icons': settings.icons,
    'data-term-palette': settings.palette,
    'data-term-crt': onOff(settings.crt),
    'data-term-blink': onOff(settings.cursorBlink),
  };
}

/** Setzt die Attribute auf <html>, solange der Rahmen steht, und entfernt sie danach wieder */
export function useRootAttributes(settings: TerminalSettings): void {
  const attributes = rootAttributes(settings);
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
