// Erzeugt aus den Schrift-Atlanten des Spiels eine TrueType-Schrift: jedes gesetzte Pixel wird ein Quadrat im Umriss.
// 1 Spielpixel = 128 Einheiten bei 1024 je Geviert; bei font-size 16px ist ein Spielpixel damit genau 2×2 CSS-Pixel.
// opentype.js baut die Tabellen (head, hhea, hmtx, cmap, name, OS/2, post); die Umrisse schreibt `toTrueType`
// als `glyf`/`loca`, weil opentype.js sonst eine CFF-Datei („OTTO“) statt einer TrueType-Datei liefert.
import opentype from 'opentype.js';
import { advanceOf, type GlyphMap } from './glyphMap.js';
import { toTrueType, type Contour } from './trueType.js';

export const UNITS_PER_EM = 1024;
export const UNITS_PER_PIXEL = 128;
export const FONT_FAMILY = 'Minecraft';
const ASCENT_PX = 7;
const DESCENT_PX = 2;
const NOTDEF_ADVANCE_PX = 6;

export interface AtlasImage {
  width: number;
  height: number;
  data: Uint8ClampedArray;
}

// Alpha-Kanal als ein Byte je Pixel
function alphaOf(img: AtlasImage): Uint8ClampedArray {
  const out = new Uint8ClampedArray(img.width * img.height);
  for (let i = 0; i < out.length; i++) out[i] = img.data[i * 4 + 3];
  return out;
}

// Umriss einer Zelle: waagrechte Läufe gesetzter Pixel je Zeile als Rechtecke, jedes im Uhrzeigersinn.
// Zeile `r` einer Zelle mit Ascent `a` liegt zwischen y = (a − r − 1)·128 und (a − r)·128.
function cellContours(
  alpha: Uint8ClampedArray,
  stride: number,
  x0: number,
  y0: number,
  w: number,
  h: number,
  ascent: number,
): Contour[] {
  const contours: Contour[] = [];
  for (let y = 0; y < h; y++) {
    let x = 0;
    while (x < w) {
      if (alpha[(y0 + y) * stride + x0 + x] === 0) {
        x++;
        continue;
      }
      let end = x;
      while (end < w && alpha[(y0 + y) * stride + x0 + end] > 0) end++;
      const top = (ascent - y) * UNITS_PER_PIXEL;
      const bottom = top - UNITS_PER_PIXEL;
      const left = x * UNITS_PER_PIXEL;
      const right = end * UNITS_PER_PIXEL;
      contours.push([
        [left, bottom],
        [left, top],
        [right, top],
        [right, bottom],
      ]);
      x = end;
    }
  }
  return contours;
}

// Derselbe Umriss als Pfad für opentype.js, damit Maße und Begrenzungen in den Tabellen stimmen
function pathOf(contours: Contour[]): opentype.Path {
  const path = new opentype.Path();
  for (const [first, ...rest] of contours) {
    path.moveTo(first[0], first[1]);
    for (const [x, y] of rest) path.lineTo(x, y);
    path.close();
  }
  return path;
}

export function buildFont(atlases: Record<string, AtlasImage>, map: GlyphMap): ArrayBuffer {
  const alphas = new Map(Object.entries(atlases).map(([k, img]) => [k, { alpha: alphaOf(img), img }]));
  const outlines: Contour[][] = [[], []];
  const glyphs: opentype.Glyph[] = [
    new opentype.Glyph({
      name: '.notdef',
      unicode: 0,
      advanceWidth: NOTDEF_ADVANCE_PX * UNITS_PER_PIXEL,
      path: new opentype.Path(),
    }),
    new opentype.Glyph({
      name: 'space',
      unicode: 32,
      advanceWidth: map.spaceAdvance * UNITS_PER_PIXEL,
      path: new opentype.Path(),
    }),
  ];
  for (const [code, cell] of map.glyphs) {
    const a = alphas.get(cell.atlas);
    if (!a) continue;
    // Das Zellenraster der Atlanten ist quadratisch (im Spiel 16×16): die Spaltenzahl ist die Zeilenzahl.
    const rows = Math.round(a.img.height / cell.height);
    const cellH = a.img.height / rows;
    const cellW = a.img.width / rows;
    const x0 = cell.col * cellW;
    const y0 = cell.row * cellH;
    const advance = advanceOf(a.alpha, cellW, cellH, a.img.width, x0, y0);
    if (advance === 0) continue;
    const contours = cellContours(a.alpha, a.img.width, x0, y0, cellW, cellH, cell.ascent);
    outlines.push(contours);
    glyphs.push(
      new opentype.Glyph({
        name: `u${code.toString(16).toUpperCase()}`,
        unicode: code,
        advanceWidth: advance * UNITS_PER_PIXEL,
        path: pathOf(contours),
      }),
    );
  }
  const font = new opentype.Font({
    familyName: FONT_FAMILY,
    styleName: 'Regular',
    unitsPerEm: UNITS_PER_EM,
    ascender: ASCENT_PX * UNITS_PER_PIXEL,
    descender: -DESCENT_PX * UNITS_PER_PIXEL,
    glyphs,
  });
  return toTrueType(font.toArrayBuffer(), outlines);
}
