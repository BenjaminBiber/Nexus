// Eigene Einstellungen des Minecraft-Themes; manifest.json hält dasselbe Schema (Test vergleicht beides)
import type { ThemeSettingsSchema } from '@nexus/theme-sdk';

export const MINECRAFT_SETTINGS = {
  sounds: { type: 'boolean', label: 'Sounds', hint: 'Klick- und Menü-Töne aus Minecraft', default: true },
} as const satisfies ThemeSettingsSchema;
