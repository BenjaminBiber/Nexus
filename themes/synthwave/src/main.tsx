import type { ThemeDefinition } from '@nexus/theme-sdk';
import { SYNTH_SETTINGS } from './settings.js';

// Schema „Synthwave“: Neon der 80er auf dunklem Violett, Sonne über Gitterhorizont. Noch ohne eigene Slots: die
// Standard-Oberfläche trägt die Farben aus theme.css.
const synthwaveTheme: ThemeDefinition = {
  id: 'synthwave',
  slots: {},
  settingsSchema: SYNTH_SETTINGS,
};

export default synthwaveTheme;
