import { Win95Frame } from '../components/win95/Win95Frame.js';
import type { ThemeDefinition } from '../theme/slots.js';

// Schema „Windows 95“: Desktop mit Fenster, Startmenü und Taskleiste statt Seitenleiste
export const win95Theme: ThemeDefinition = {
  id: 'win95',
  slots: {
    Shell: Win95Frame,
  },
};
