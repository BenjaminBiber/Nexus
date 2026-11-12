import type { ThemeDefinition } from '@nexus/theme-sdk';
import { draculaMermaid } from './mermaid.js';
import { DRACULA } from './palette.js';
import { DraculaProgress } from './Progress.js';

// Schema „Dracula“: die Standard-Oberfläche in der Dracula-Palette (Farben in theme.css). Eigener Code nur für die
// Mermaid-Diagramme und den Fortschrittsbalken; Rahmen, Symbole und alles andere bleiben Standard.
const draculaTheme: ThemeDefinition = {
  id: 'dracula',
  slots: { Progress: DraculaProgress },
  mermaid: () => draculaMermaid(DRACULA, true),
};

export default draculaTheme;
