// Symbole: jede Glyphe hat eine Nerd-Font- und eine Unicode-Form; die Nerd-Formen liegen im privaten Bereich der Symbolschrift
import { describe, expect, it } from 'vitest';
import { GLYPHS, glyphFor } from '../src/glyphs.js';

const NAV = ['board', 'dashboard', 'inbox', 'sessions', 'questions', 'docs', 'search', 'guide', 'settings'];
const CORE = ['doc', 'plan', 'glossary', 'adr', 'file', 'folder', 'folderOpen', 'empty', 'edit', 'comment'];

describe('Symbole „Terminal“', () => {
  it('deckt alle Navigations- und Kern-Symbole des SDK ab', () => {
    for (const name of [...NAV, ...CORE]) {
      expect(GLYPHS.nerd).toHaveProperty(name);
      expect(GLYPHS.unicode).toHaveProperty(name);
    }
  });

  it('nimmt für Nerd Font nur Zeichen aus den privaten Bereichen (dort liegen die Glyphen der Symbolschrift)', () => {
    for (const glyph of Object.values(GLYPHS.nerd)) {
      const code = glyph.codePointAt(0)!;
      expect(code >= 0xe000 && code <= 0xf8ff || code >= 0xf0000).toBe(true);
    }
  });

  it('wählt die Form nach der Einstellung', () => {
    expect(glyphFor('docs', 'nerd')).toBe('\u{f405}');
    expect(glyphFor('docs', 'unicode')).toBe('≡');
    expect(glyphFor('docs', 'off')).toBeNull();
  });
});
