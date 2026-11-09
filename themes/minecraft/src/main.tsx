// Einstieg des Minecraft-Themes: Vite baut daraus dist/main.js (ESM); React und das SDK liefert Nexus zur Laufzeit
import type { ThemeDefinition } from '@nexus/theme-sdk';
import { MinecraftShell } from './MinecraftShell.js';
import { NavIcon } from './NavIcon.js';
import { THEME_ID } from './resources.js';
import { MINECRAFT_SETTINGS } from './settings.js';

const theme: ThemeDefinition = { id: THEME_ID, slots: { Shell: MinecraftShell, NavIcon }, settingsSchema: MINECRAFT_SETTINGS };

export default theme;
