import type { ThemeSettingsSchema } from '@nexus/theme-sdk';

/** Ereignis nach dem Speichern eigener Theme-Einstellungen; Detail `{ themeId }` */
export const THEME_SETTINGS_EVENT = 'nexus-theme-settings';

// Werte je Schema-Schlüssel: gespeicherter Wert, wenn Typ (und bei Auswahl die Option) passt, sonst der Default
export function resolveThemeSettings(schema: ThemeSettingsSchema | undefined, stored: Record<string, unknown>): Record<string, boolean | string> {
  const values: Record<string, boolean | string> = {};
  for (const [key, setting] of Object.entries(schema ?? {})) {
    const value = stored[key];
    if (setting.type === 'boolean') values[key] = typeof value === 'boolean' ? value : setting.default;
    else values[key] = typeof value === 'string' && setting.options.some((o) => o.value === value) ? value : setting.default;
  }
  return values;
}
