import type { ThemeDefinition } from '@nexus/theme-sdk';
import { TerminalFrame } from './TerminalFrame.js';

// Schema „Terminal“: dunkles Terminal mit Prompt-Zeile (Befehle wie help, cd, open), Navigation als Liste,
// Statuszeile wie in vim/tmux und Festbreitenschrift überall
const terminalTheme: ThemeDefinition = {
  id: 'terminal',
  slots: {
    Shell: TerminalFrame,
  },
};

export default terminalTheme;
