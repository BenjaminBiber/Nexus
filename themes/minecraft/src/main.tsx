// Einstieg des Minecraft-Themes: Vite baut daraus dist/main.js (ESM); React und das SDK liefert Nexus zur Laufzeit
import type { ThemeDefinition } from '@nexus/theme-sdk';
import { MinecraftShell } from './MinecraftShell.js';
import { NavIcon } from './NavIcon.js';
import { THEME_ID } from './resources.js';
import { MINECRAFT_SETTINGS } from './settings.js';
import { McButtonSlot } from './slots/McButtonSlot.js';
import { McCard } from './slots/McCard.js';
import { McDialog } from './slots/McDialog.js';
import { McInput } from './slots/McInput.js';
import { McPanel } from './slots/McPanel.js';
import { McProgress } from './slots/McProgress.js';
import { McStart } from './slots/McStart.js';

const theme: ThemeDefinition = {
  id: THEME_ID,
  slots: {
    Shell: MinecraftShell,
    NavIcon,
    Button: McButtonSlot,
    Input: McInput,
    Progress: McProgress,
    Card: McCard,
    Panel: McPanel,
    Dialog: McDialog,
    StartAnimation: McStart,
  },
  settingsSchema: MINECRAFT_SETTINGS,
};

export default theme;

// Die Schrift-Erzeugung gehört zur Oberfläche des Moduls, damit opentype.js in main.js gebündelt liegt
// und das Theme zur Laufzeit keine zweite Datei nachladen muss.
export { buildFont, FONT_FAMILY, UNITS_PER_EM, UNITS_PER_PIXEL, type AtlasImage } from './font/buildFont.js';
