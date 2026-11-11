import type { ThemeDefinition } from '@nexus/theme-sdk';
import { GTA_SETTINGS } from './settings.js';
import { GtaShell } from './GtaShell.js';

// Schema „GTA“: Nexus als Teil des Spiels, wahlweise im Look von GTA V (Pausenmenü) oder San Andreas (Menüliste). Der
// Rahmen ist eine eigene Shell; die Seiten des Kerns gestaltet theme.css im selben Look.
const gtaTheme: ThemeDefinition = {
  id: 'gta',
  slots: { Shell: GtaShell },
  settingsSchema: GTA_SETTINGS,
};

export default gtaTheme;
