import { DiskPie } from '../components/win95/DiskPie.js';
import { Win95Frame } from '../components/win95/Win95Frame.js';
import { Win95Icon, type Win95IconName } from '../components/win95/Win95Icons.js';
import type { NavIconName, ThemeDefinition } from '../theme/slots.js';

// Pixel-Symbol je Navigationssymbol
const ICONS: Record<NavIconName, Win95IconName> = {
  board: 'folder',
  dashboard: 'chart',
  inbox: 'inbox',
  sessions: 'prompt',
  questions: 'question',
  docs: 'document',
  search: 'magnifier',
  guide: 'help',
  settings: 'computer',
};

// Schema „Windows 95“: Desktop mit Fenster, Startmenü und Taskleiste statt Seitenleiste, Pixel-Symbole
export const win95Theme: ThemeDefinition = {
  id: 'win95',
  slots: {
    Shell: Win95Frame,
    NavIcon: ({ name }) => <Win95Icon name={ICONS[name]} />,
    Progress: ({ done, total, label }) => <DiskPie done={done} total={total} label={label} />,
  },
};
