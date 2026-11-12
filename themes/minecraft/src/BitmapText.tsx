// Minecraft-Bitmap-Schrift aus ascii.png (16×16 Zellen à 8×8): Glyphen als Hintergrund-Ausschnitte des Atlas.
// Der Text selbst bleibt im DOM (transparent darüber), damit Screenreader, Suche und Kopieren funktionieren.
import { useEffect, useState } from 'react';
import { useResourceUrl } from './context.js';
import { FONT_ATLAS } from './resources.js';

/** Kantenlänge einer Zelle im Atlas */
const CELL = 8;
/** Vergrößerung der Glyphen auf dem Bildschirm */
export const SCALE = 2;
/** Breite je Glyphe, solange der Atlas nicht vermessen ist (ohne Canvas, z. B. in jsdom) */
export const DEFAULT_GLYPH_WIDTH = 5;
/** Breite des Leerzeichens wie im Spiel (Vorschub 4 mit 1 Abstand) */
const SPACE_WIDTH = 3;

export interface GlyphRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Zelle eines Zeichens im Atlas; null außerhalb von ASCII */
export function glyphRect(char: string): GlyphRect | null {
  const code = char.charCodeAt(0);
  if (char.length !== 1 || code > 127) return null;
  return { x: (code % 16) * CELL, y: Math.floor(code / 16) * CELL, width: CELL, height: CELL };
}

/** Breite je Zeichencode (0–255) in Atlas-Einheiten: letzte nicht transparente Spalte der Zelle + 1 */
export function measureGlyphWidths(image: { width: number; height: number; data: ArrayLike<number> }): number[] {
  const cellW = image.width / 16;
  const cellH = image.height / 16;
  const widths: number[] = [];
  for (let code = 0; code < 256; code++) {
    const left = (code % 16) * cellW;
    const top = Math.floor(code / 16) * cellH;
    let last = -1;
    for (let y = top; y < top + cellH; y++) {
      for (let x = left + cellW - 1; x > left + last; x--) {
        if (image.data[(y * image.width + x) * 4 + 3] > 0) {
          last = x - left;
          break;
        }
      }
    }
    // Atlas in höherer Auflösung (Ressourcenpaket) auf das 8er-Raster zurückrechnen
    widths.push(Math.ceil(((last + 1) * CELL) / cellW));
  }
  widths[32] = SPACE_WIDTH;
  return widths;
}

const measured = new Map<string, number[]>();
const pending = new Map<string, Promise<number[] | null>>();

// Atlas einmal per Canvas vermessen; null ohne Canvas oder wenn das Bild nicht lädt
function loadWidths(url: string): Promise<number[] | null> {
  return new Promise((resolve) => {
    if (typeof Image === 'undefined') return resolve(null);
    const img = new Image();
    img.onload = () => {
      try {
        const canvas = document.createElement('canvas');
        canvas.width = img.naturalWidth;
        canvas.height = img.naturalHeight;
        const ctx = canvas.getContext('2d');
        if (!ctx) return resolve(null);
        ctx.drawImage(img, 0, 0);
        resolve(measureGlyphWidths(ctx.getImageData(0, 0, canvas.width, canvas.height)));
      } catch {
        resolve(null);
      }
    };
    img.onerror = () => resolve(null);
    img.src = url;
  });
}

// Gecachte Breiten des Atlas; bis zur Vermessung null (dann gilt DEFAULT_GLYPH_WIDTH)
function useGlyphWidths(url: string | undefined): number[] | null {
  const [, setTick] = useState(0);
  useEffect(() => {
    if (!url || measured.has(url)) return;
    let live = true;
    let job = pending.get(url);
    if (!job) {
      job = loadWidths(url).then((widths) => {
        if (widths) measured.set(url, widths);
        return widths;
      });
      pending.set(url, job);
    }
    void job.then((widths) => {
      if (live && widths) setTick((n) => n + 1);
    });
    return () => {
      live = false;
    };
  }, [url]);
  return url ? (measured.get(url) ?? null) : null;
}

/** Text in der Minecraft-Schrift; ohne Atlas (Laden, offline) die Rückfallschrift `mc-text--fallback` */
export function McText({ children, className }: { children: string; className?: string }) {
  const atlas = useResourceUrl(FONT_ATLAS);
  const widths = useGlyphWidths(atlas);
  const classes = ['mc-text', atlas ? '' : 'mc-text--fallback', className ?? ''].filter(Boolean).join(' ');
  if (!atlas) {
    return (
      <span className={classes} aria-label={children}>
        {children}
      </span>
    );
  }
  return (
    <span className={classes} aria-label={children}>
      <span className="mc-glyphs" aria-hidden="true">
        {Array.from(children).map((char, i) => {
          const rect = glyphRect(char);
          // Umlaute und Sonderzeichen fehlen in ascii.png: dieses Zeichen per CSS (attr) in der Rückfallschrift
          if (!rect) return <span key={i} className="mc-glyph-fallback" data-char={char} />;
          const width = char === ' ' ? SPACE_WIDTH : widths?.[char.charCodeAt(0)] || DEFAULT_GLYPH_WIDTH;
          return (
            <span
              key={i}
              className="mc-glyph"
              style={{ width: `${width * SCALE}px`, backgroundPosition: `${-rect.x * SCALE}px ${-rect.y * SCALE}px` }}
            />
          );
        })}
      </span>
      <span className="mc-text-src">{children}</span>
    </span>
  );
}
