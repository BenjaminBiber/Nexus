import type { ThemeDefinition } from '@nexus/theme-sdk';
import { SYNTH_SETTINGS } from './settings.js';
import { SynthShell } from './SynthShell.js';
import { NeonIcon } from './NeonIcon.js';

// Schema „Synthwave“: Neon der 80er auf dunklem Violett, Sonne über Gitterhorizont. Der Rahmen ist ein Kassettendeck mit
// der Navigation als Titelliste; alles andere trägt die Standard-Oberfläche mit den Farben aus theme.css.
const synthwaveTheme: ThemeDefinition = {
  id: 'synthwave',
  slots: {
    Shell: SynthShell,
    NavIcon: ({ name }) => <NeonIcon name={name} />,
    Icon: ({ name }) => <NeonIcon name={name} />,
  },
  settingsSchema: SYNTH_SETTINGS,
};

export default synthwaveTheme;
