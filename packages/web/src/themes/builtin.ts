import { registerTheme } from '../theme/registry.js';
import { win95Theme } from './win95.js';

// Eingebaute Themes mit eigenen Slots; Hell und Dunkel ändern nur CSS und brauchen keinen Eintrag
registerTheme(win95Theme);
