import type { ThemeDefinition } from '@nexus/theme-sdk';
import { draculaMermaid } from './mermaid.js';
import { ALUCARD } from './palette.js';
import { DraculaProgress } from './Progress.js';

// Schema „Alucard“: die Standard-Oberfläche in der hellen Dracula-Palette (Farben in theme.css). Eigener Code nur für die
// Mermaid-Diagramme und den Fortschrittsbalken; Rahmen, Symbole und alles andere bleiben Standard.
const alucardTheme: ThemeDefinition = {
  id: 'alucard',
  slots: { Progress: DraculaProgress },
  mermaid: () => draculaMermaid(ALUCARD, false),
};

export default alucardTheme;
