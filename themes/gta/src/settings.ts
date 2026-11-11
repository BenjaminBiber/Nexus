// Eigene Einstellungen des GTA-Themes; manifest.json hält dasselbe Schema (Test vergleicht beides).
// Die Shell spiegelt die Werte als data-gta-* auf <html>, damit theme.css auch Dialoge und Popovers außerhalb des Rahmens trifft.
// Stil und Ladebildschirm-Schalter landen zusätzlich in localStorage: der Ladebildschirm läuft vor der Shell, und beim
// Laden der Einstellungen soll kein falscher Rahmen aufblitzen.
import { useEffect, useMemo } from 'react';
import { useThemeSettings, type ThemeSettingsSchema } from '@nexus/theme-sdk';

export const GTA_SETTINGS = {
  style: {
    type: 'select',
    label: 'Stil',
    hint: 'Rahmen im Look von GTA V (Pausenmenü) oder San Andreas (Menüliste)',
    options: [
      { value: 'v', label: 'GTA V' },
      { value: 'sa', label: 'San Andreas' },
    ],
    default: 'v',
  },
  accent: {
    type: 'select',
    label: 'Akzentfarbe',
    hint: 'Farbe der Auswahl in Menü und Reitern',
    options: [
      { value: 'blau', label: 'Blau' },
      { value: 'gruen', label: 'Grün' },
      { value: 'orange', label: 'Orange' },
    ],
    default: 'blau',
  },
  radar: { type: 'boolean', label: 'Radar', hint: 'Minimap bzw. Radar unten links mit den Tickets des Boards', default: true },
  banners: { type: 'boolean', label: 'Banner', hint: 'Mission Passed, Busted, Wasted und Handy-Meldungen zu neuen Fragen', default: true },
  boot: { type: 'boolean', label: 'Ladebildschirm', hint: 'Ladebildschirm beim Öffnen der Oberfläche', default: true },
  cheats: { type: 'boolean', label: 'Cheats', hint: 'Tastenfolgen wie HESOYAM; ändern nur die Anzeige', default: true },
} as const satisfies ThemeSettingsSchema;

type Schema = typeof GTA_SETTINGS;

/** Gültige Werte aller Einstellungen */
export type GtaSettings = {
  [K in keyof Schema]: Schema[K] extends { type: 'select'; options: readonly { value: infer V }[] } ? V : boolean;
};

type Style = GtaSettings['style'];

// Nimmt gespeicherte Werte nur, wenn sie zum Schema passen; sonst gilt der Default
export function resolveSettings(values: Record<string, unknown>): GtaSettings {
  const out: Record<string, boolean | string> = {};
  for (const [key, setting] of Object.entries(GTA_SETTINGS) as [string, Schema[keyof Schema]][]) {
    const value = values[key];
    if (setting.type === 'boolean') out[key] = typeof value === 'boolean' ? value : setting.default;
    else out[key] = typeof value === 'string' && setting.options.some((o) => o.value === value) ? value : setting.default;
  }
  return out as GtaSettings;
}

/** Einstellungen des Themes, immer vollständig (während des Ladens die Defaults) */
export function useGtaSettings(): GtaSettings & { loading: boolean } {
  const { values, loading } = useThemeSettings();
  return useMemo(() => ({ ...resolveSettings(values ?? {}), loading }), [values, loading]);
}

export const STYLE_KEY = 'nexus-gta-style';
export const BOOT_KEY = 'nexus-gta-boot';

function read(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // privates Fenster oder gesperrter Speicher: dann eben ohne
  }
}

/** Merkt sich den Stil für das nächste Laden */
export function rememberStyle(style: Style): void {
  write(STYLE_KEY, style);
}

/** Gemerkter Stil; ohne oder mit ungültigem Eintrag GTA V */
export function rememberedStyle(): Style {
  return read(STYLE_KEY) === 'sa' ? 'sa' : 'v';
}

/** Merkt sich, ob der Ladebildschirm laufen soll */
export function rememberBoot(on: boolean): void {
  write(BOOT_KEY, on ? 'on' : 'off');
}

/** Gemerkter Schalter des Ladebildschirms; ohne Eintrag an */
export function rememberedBoot(): boolean {
  return read(BOOT_KEY) !== 'off';
}

const onOff = (value: boolean) => (value ? 'on' : 'off');

/** Attribute für <html>, auf die theme.css reagiert; der Stil kommt aufgelöst herein (siehe useActiveStyle) */
export function rootAttributes(settings: GtaSettings, style: Style): Record<string, string> {
  return {
    'data-gta-style': style,
    'data-gta-accent': settings.accent,
    'data-gta-radar': onOff(settings.radar),
  };
}

// Solange die Einstellungen laden, gilt der gemerkte Stil, sonst die Einstellung
const activeStyle = (settings: GtaSettings & { loading: boolean }): Style => (settings.loading ? rememberedStyle() : settings.style);

/** Aufgelöster Stil für Komponenten: geladen die Einstellung, sonst der gemerkte */
export function useActiveStyle(): Style {
  return activeStyle(useGtaSettings());
}

/** Setzt die Attribute auf <html>, solange der Rahmen steht, und entfernt sie danach wieder */
export function useRootAttributes(settings: GtaSettings & { loading: boolean }): void {
  const key = JSON.stringify(rootAttributes(settings, activeStyle(settings)));
  useEffect(() => {
    const root = document.documentElement;
    const entries = Object.entries(JSON.parse(key) as Record<string, string>);
    for (const [name, value] of entries) root.setAttribute(name, value);
    return () => {
      for (const [name] of entries) root.removeAttribute(name);
    };
  }, [key]);
}
