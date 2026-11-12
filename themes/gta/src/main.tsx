import type { ThemeDefinition } from '@nexus/theme-sdk';
import { GTA_SETTINGS } from './settings.js';
import { GtaShell } from './GtaShell.js';
import { gtaMermaid } from './mermaid.js';
import { GtaIcon, GtaNavIcon, GtaProgress, GtaStartAnimation } from './slots.js';

// Schema „GTA“: Nexus als Teil des Spiels, wahlweise im Look von GTA V (Pausenmenü) oder San Andreas (Menüliste). Der
// Rahmen ist eine eigene Shell, dazu Symbole, Fortschrittsbalken und Ladebildschirm im Stil; die Seiten des Kerns
// gestaltet theme.css im selben Look, Mermaid-Diagramme folgen Stil und Akzentfarbe.
const gtaTheme: ThemeDefinition = {
  id: 'gta',
  slots: {
    Shell: GtaShell,
    NavIcon: GtaNavIcon,
    Icon: GtaIcon,
    Progress: GtaProgress,
    StartAnimation: GtaStartAnimation,
  },
  settingsSchema: GTA_SETTINGS,
  mermaid: gtaMermaid,
};

export default gtaTheme;
