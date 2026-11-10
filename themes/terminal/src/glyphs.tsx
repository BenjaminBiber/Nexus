// Symbole des Terminal-Themes: Glyphen aus „Symbols Nerd Font Mono“ (Teilmenge in assets/fonts/term-symbols.woff2)
// oder schlichte Unicode-Zeichen, je nach Einstellung „Symbole“. Wer hier eine Glyphe ergänzt, muss die Schrift neu
// schneiden (Codepoints in GLYPHS.nerd, siehe Kommentar in theme.css).
import type { IconName, NavIconName } from '@nexus/theme-sdk';
import { useTerminalSettings, type TerminalSettings } from './settings.js';

/** Alle Symbole, die das Theme zeigt: Navigation, Kern-Symbole und Statuszeile */
export type GlyphName = NavIconName | IconName | 'clock' | 'claude';

// Nerd-Font-Name als Kommentar (glyphnames.json von Nerd Fonts), Codepoint als Zeichen
const NERD: Record<GlyphName, string> = {
  board: '\u{f0db}', // nf-fa-columns
  dashboard: '\u{f056e}', // nf-md-view_dashboard
  inbox: '\u{f01c}', // nf-fa-inbox
  sessions: '\u{f489}', // nf-oct-terminal
  questions: '\u{f420}', // nf-oct-question
  docs: '\u{f405}', // nf-oct-book
  search: '\u{f422}', // nf-oct-search
  guide: '\u{f494}', // nf-oct-mortar_board
  settings: '\u{f423}', // nf-oct-gear
  doc: '\u{f09ee}', // nf-md-file_document_outline
  plan: '\u{f45e}', // nf-oct-checklist
  glossary: '\u{f061d}', // nf-md-book_alphabet
  adr: '\u{f495}', // nf-oct-law
  file: '\u{f4a5}', // nf-oct-file
  folder: '\u{f07b}', // nf-fa-folder
  folderOpen: '\u{f07c}', // nf-fa-folder_open
  empty: '\u{f02a0}', // nf-md-ghost
  edit: '\u{f448}', // nf-oct-pencil
  comment: '\u{f41f}', // nf-oct-comment
  clock: '\u{f43a}', // nf-oct-clock
  claude: '\u{f167a}', // nf-md-robot_outline
};

// Ersatz ohne Symbolschrift; U+FE0E hält Zeichen wie ⚙ als Text statt als Emoji
const UNICODE: Record<GlyphName, string> = {
  board: '▦',
  dashboard: '◫',
  inbox: '✉\u{fe0e}',
  sessions: '❯',
  questions: '?',
  docs: '≡',
  search: '⌕',
  guide: '◇',
  settings: '⚙\u{fe0e}',
  doc: '¶',
  plan: '☐',
  glossary: '§',
  adr: '⚖\u{fe0e}',
  file: '·',
  folder: '▸',
  folderOpen: '▾',
  empty: '∅',
  edit: '✎',
  comment: '❝',
  clock: '◷',
  claude: '✻',
};

export const GLYPHS = { nerd: NERD, unicode: UNICODE } as const;

/** Zeichen eines Symbols je Einstellung; null bei „keine“ */
export function glyphFor(name: GlyphName, mode: TerminalSettings['icons']): string | null {
  return mode === 'off' ? null : GLYPHS[mode][name];
}

/** Ein Symbol als Text; Farbe und Schrift setzt theme.css über data-glyph */
export function TermGlyph({ name }: { name: GlyphName }) {
  const { icons } = useTerminalSettings();
  const glyph = glyphFor(name, icons);
  if (glyph == null) return null;
  return (
    <span className={`term-glyph term-glyph-${icons}`} data-glyph={name} aria-hidden="true">
      {glyph}
    </span>
  );
}
