// Schrift-Definition des Spiels (font/include/default.json, space.json): Zeichen → Zelle im Atlas und Vorschub-Regel.
// Zellen sind in `chars` zeilenweise als Strings aus Codepoints notiert; Leerzeichen und U+0000 in `chars` sind keine Glyphen.
export interface GlyphCell {
  atlas: string;
  col: number;
  row: number;
  ascent: number;
  height: number;
}
export interface GlyphMap {
  glyphs: Map<number, GlyphCell>;
  spaceAdvance: number;
  atlases: string[];
}
interface BitmapProvider {
  type: 'bitmap';
  file: string;
  ascent: number;
  height?: number;
  chars: string[];
}
interface SpaceProvider {
  type: 'space';
  advances: Record<string, number>;
}
export interface FontJson {
  providers: Array<BitmapProvider | SpaceProvider | { type: string }>;
}
export type SpaceJson = FontJson;

/** `minecraft:font/ascii.png` → JAR-Pfad der Textur */
export function atlasEntry(file: string): string {
  const name = file.replace(/^minecraft:/, '');
  return `assets/minecraft/textures/${name}`;
}

/** Zeichen → Zelle: der erste Provider, der ein Zeichen nennt, gewinnt (wie im Spiel) */
export function parseGlyphMap(defaultJson: FontJson, spaceJson: SpaceJson): GlyphMap {
  const glyphs = new Map<number, GlyphCell>();
  const atlases: string[] = [];
  for (const p of defaultJson.providers) {
    if (p.type !== 'bitmap') continue;
    const { file, ascent, height = 8, chars } = p as BitmapProvider;
    const atlas = atlasEntry(file);
    atlases.push(atlas);
    chars.forEach((line, row) => {
      Array.from(line).forEach((ch, col) => {
        const code = ch.codePointAt(0)!;
        if (code === 0 || code === 32 || glyphs.has(code)) return;
        glyphs.set(code, { atlas, col, row, ascent, height });
      });
    });
  }
  const space = spaceJson.providers.find((p): p is SpaceProvider => p.type === 'space');
  return { glyphs, spaceAdvance: space?.advances[' '] ?? 4, atlases };
}

/** Vorschub einer Zelle wie im Spiel: letzte Spalte mit Alpha > 0, plus 1, plus 1 px Abstand; leere Zelle 0 */
export function advanceOf(
  alpha: Uint8ClampedArray,
  width: number,
  height: number,
  stride: number,
  offsetX: number,
  offsetY: number,
): number {
  let last = -1;
  for (let y = 0; y < height; y++)
    for (let x = width - 1; x > last; x--) {
      if (alpha[(offsetY + y) * stride + offsetX + x] > 0) {
        last = x;
        break;
      }
    }
  return last < 0 ? 0 : last + 2;
}
