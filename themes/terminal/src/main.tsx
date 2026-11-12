import type { ThemeDefinition } from '@nexus/theme-sdk';
import { BootScreen } from './BootScreen.js';
import { TermGlyph } from './glyphs.js';
import { terminalMermaid } from './mermaid.js';
import { TERMINAL_SETTINGS } from './settings.js';
import { TerminalFrame } from './TerminalFrame.js';
import { TerminalProgress } from './TerminalProgress.js';

// Schema „Terminal“: dunkles Terminal mit Prompt-Zeile im Powerline-Stil (Befehle wie help, cd, fetch), Navigation als
// Ausgabe von „ls --icons“, Statuszeile wie in vim/tmux, Boot-Meldungen beim Start und Festbreitenschrift überall.
// Schrift, Symbole, Farbschema und Effekte stehen unter Einstellungen → Darstellung.
const terminalTheme: ThemeDefinition = {
  id: 'terminal',
  slots: {
    Shell: TerminalFrame,
    NavIcon: ({ name }) => <TermGlyph name={name} />,
    Icon: ({ name }) => <TermGlyph name={name} />,
    Progress: TerminalProgress,
    StartAnimation: BootScreen,
  },
  settingsSchema: TERMINAL_SETTINGS,
  mermaid: terminalMermaid,
};

export default terminalTheme;
