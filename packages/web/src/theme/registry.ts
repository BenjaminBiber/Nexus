import type { ThemeDefinition } from '@nexus/theme-sdk';

const themes = new Map<string, ThemeDefinition>();

// Meldet ein Theme an; eines mit gleicher ID wird ersetzt
export function registerTheme(def: ThemeDefinition): void {
  themes.set(def.id, def);
}

// Meldet ein Theme ab (z. B. nach dem Deinstallieren)
export function unregisterTheme(id: string): void {
  themes.delete(id);
}

// Angemeldetes Theme zu einer ID oder undefined
export function getTheme(id: string): ThemeDefinition | undefined {
  return themes.get(id);
}
