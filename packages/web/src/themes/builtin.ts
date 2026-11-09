import { registerTheme } from '../theme/registry.js';
import { win95Theme } from './win95.js';

// Eingebaute Themes mit eigenen Slots; Hell, Dunkel, Signal und Pastell ändern nur CSS und brauchen keinen Eintrag
registerTheme(win95Theme);
