// Schrift aus den Atlanten: Maße des Spiels, Vorschub und Umriss je Zelle – Atlanten synthetisch, keine Mojang-Datei
import opentype from 'opentype.js';
import { describe, expect, it } from 'vitest';
import { buildFont, UNITS_PER_EM } from '../src/font/buildFont.js';
import type { GlyphMap } from '../src/font/glyphMap.js';

// Künstlicher Atlas: eine Zelle breit, RGBA; `on` nennt gesetzte Pixel (x, y)
function atlas(width: number, height: number, on: Array<[number, number]>) {
  const data = new Uint8ClampedArray(width * height * 4);
  for (const [x, y] of on) data[(y * width + x) * 4 + 3] = 255;
  return { width, height, data };
}
// 8×8-Kreuz: Spalten 0–4 in Zeile 3, Zeilen 0–6 in Spalte 2
const kreuz = atlas(8, 8, [
  ...[0, 1, 2, 3, 4].map((x): [number, number] => [x, 3]),
  ...[0, 1, 2, 3, 4, 5, 6].map((y): [number, number] => [2, y]),
]);
// 9×12-Zelle: Pixel in Zeile 11 (unterste) und Zeile 0
const hoch = atlas(9, 12, [
  [0, 11],
  [0, 0],
]);
const map: GlyphMap = {
  glyphs: new Map([
    ['A'.codePointAt(0)!, { atlas: 'klein', col: 0, row: 0, ascent: 7, height: 8 }],
    ['ä'.codePointAt(0)!, { atlas: 'gross', col: 0, row: 0, ascent: 10, height: 12 }],
  ]),
  spaceAdvance: 4,
  atlases: ['klein', 'gross'],
};

describe('buildFont', () => {
  const bytes = buildFont({ klein: kreuz, gross: hoch }, map);
  const font = opentype.parse(bytes);

  it('ist eine TrueType-Datei mit den Maßen des Spiels', () => {
    expect(Array.from(new Uint8Array(bytes.slice(0, 4)))).toEqual([0, 1, 0, 0]);
    expect(font.unitsPerEm).toBe(UNITS_PER_EM);
    expect(font.ascender).toBe(896);
    expect(font.descender).toBe(-256);
  });

  it('setzt Vorschub und Umriss nach der Spielregel', () => {
    const a = font.charToGlyph('A');
    expect(a.advanceWidth).toBe(6 * 128);
    const box = a.getBoundingBox();
    expect([box.x1, box.y1, box.x2, box.y2]).toEqual([0, 0, 5 * 128, 7 * 128]);
    const ae = font.charToGlyph('ä');
    expect(ae.getBoundingBox().y1).toBe(-2 * 128);
    expect(ae.getBoundingBox().y2).toBe(10 * 128);
    expect(font.charToGlyph(' ').advanceWidth).toBe(4 * 128);
    expect(font.glyphs.get(0).advanceWidth).toBe(6 * 128);
  });

  it('schreibt die Umrisse als Rechtecke und eine stimmige Prüfsumme', () => {
    // Das Kreuz: ein langer Lauf in Zeile 3, dazu je ein Pixel in den Zeilen 0–2 und 4–6
    const konturen = font.charToGlyph('A').path.commands.filter((c) => c.type === 'Z');
    expect(konturen).toHaveLength(7);
    // Über die ganze Datei summiert ergeben die 32-Bit-Wörter den von TrueType vorgeschriebenen Wert
    const words = new DataView(bytes);
    let sum = 0;
    for (let at = 0; at < bytes.byteLength; at += 4) sum = (sum + words.getUint32(at)) >>> 0;
    expect(sum).toBe(0xb1b0afba);
  });
});
