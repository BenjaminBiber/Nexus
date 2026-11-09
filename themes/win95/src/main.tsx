import { DiskPie } from './DiskPie.js';
import { Win95Frame } from './Win95Frame.js';
import { WIN95_NAV_ICONS, Win95Icon } from './Win95Icons.js';
import type { ThemeDefinition } from '@nexus/theme-sdk';

// Schema „Windows 95“: Desktop mit Fenster, Startmenü und Taskleiste statt Seitenleiste, Pixel-Symbole
const win95Theme: ThemeDefinition = {
  id: 'win95',
  slots: {
    Shell: Win95Frame,
    NavIcon: ({ name }) => <Win95Icon name={WIN95_NAV_ICONS[name]} />,
    Progress: ({ done, total, label }) => <DiskPie done={done} total={total} label={label} />,
  },
};

export default win95Theme;
