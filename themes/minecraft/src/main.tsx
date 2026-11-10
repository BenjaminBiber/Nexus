// Einstieg des Minecraft-Themes: Vite baut daraus dist/main.js (ESM); React und das SDK liefert Nexus zur Laufzeit
import type { ThemeDefinition } from '@nexus/theme-sdk';
import { MinecraftShell } from './MinecraftShell.js';
import { NavIcon } from './NavIcon.js';
import { THEME_ID } from './resources.js';
import { MINECRAFT_SETTINGS } from './settings.js';
import { McButtonSlot } from './slots/McButtonSlot.js';
import { McInput } from './slots/McInput.js';

const theme: ThemeDefinition = {
  id: THEME_ID,
  slots: { Shell: MinecraftShell, NavIcon, Button: McButtonSlot, Input: McInput },
  settingsSchema: MINECRAFT_SETTINGS,
};

export default theme;

// Die Schrift-Erzeugung gehört zur Oberfläche des Moduls, damit opentype.js in main.js gebündelt liegt
// und das Theme zur Laufzeit keine zweite Datei nachladen muss.
export { buildFont, FONT_FAMILY, UNITS_PER_EM, UNITS_PER_PIXEL, type AtlasImage } from './font/buildFont.js';
