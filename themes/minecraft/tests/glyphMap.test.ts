// Prüft die Glyphenkarte: Zeichen → Zelle im Atlas, Vorschub des Leerzeichens und die Vorschub-Regel des Spiels.
// Die Schrift-Definition ist hier selbst geschrieben (Format des Spiels, kein Mojang-Inhalt).
import { describe, expect, it } from 'vitest';
import { advanceOf, atlasEntry, parseGlyphMap } from '../src/font/glyphMap.js';

// Mini-Definition im Format des Spiels, selbst geschrieben
const DEFAULT = {
  providers: [
    { type: 'bitmap', file: 'minecraft:font/klein.png', ascent: 7, chars: ['AB', 'C '] },
    { type: 'bitmap', file: 'minecraft:font/gross.png', ascent: 10, height: 12, chars: ['äA'] },
    { type: 'space', advances: { '‌': 0 } },
  ],
};
const SPACE = { providers: [{ type: 'space', advances: { ' ': 4, '‌': 0 } }] };

describe('parseGlyphMap', () => {
  const map = parseGlyphMap(DEFAULT, SPACE);
  it('ordnet Zeichen ihrer Zelle zu, erster Provider gewinnt', () => {
    expect(map.glyphs.get('A'.codePointAt(0)!)).toEqual({
      atlas: 'assets/minecraft/textures/font/klein.png',
      col: 0,
      row: 0,
      ascent: 7,
      height: 8,
    });
    expect(map.glyphs.get('C'.codePointAt(0)!)).toMatchObject({ col: 0, row: 1 });
    expect(map.glyphs.get('ä'.codePointAt(0)!)).toEqual({
      atlas: 'assets/minecraft/textures/font/gross.png',
      col: 0,
      row: 0,
      ascent: 10,
      height: 12,
    });
  });
  it('Leerzeichen hat Vorschub 4 und ist keine Glyphe', () => {
    expect(map.spaceAdvance).toBe(4);
    expect(map.glyphs.has(32)).toBe(false);
  });
  it('listet die Atlanten in Reihenfolge', () => {
    expect(map.atlases).toEqual([
      'assets/minecraft/textures/font/klein.png',
      'assets/minecraft/textures/font/gross.png',
    ]);
  });
});

describe('advanceOf', () => {
  it('ist letzte gesetzte Spalte + 2, leere Zelle 0', () => {
    // 4×2-Zelle, Alpha je Pixel; Spalten 0–2 der ersten Zeile gesetzt
    const alpha = new Uint8ClampedArray([255, 255, 255, 0, 0, 0, 0, 0]);
    expect(advanceOf(alpha, 4, 2, 4, 0, 0)).toBe(4);
    expect(advanceOf(new Uint8ClampedArray(8), 4, 2, 4, 0, 0)).toBe(0);
  });
});

it('atlasEntry bildet den Ressourcennamen auf den JAR-Pfad ab', () => {
  expect(atlasEntry('minecraft:font/ascii.png')).toBe('assets/minecraft/textures/font/ascii.png');
});
