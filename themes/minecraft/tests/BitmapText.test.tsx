// @vitest-environment jsdom
// Bitmap-Schrift aus ascii.png: Glyphen-Zellen, Breiten aus dem Atlas, zugänglicher Text und Rückfallschrift
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { DEFAULT_GLYPH_WIDTH, McText, glyphRect, measureGlyphWidths } from '../src/BitmapText.js';
import { ResourcesProvider } from '../src/context.js';
import { FONT_ATLAS } from '../src/resources.js';

afterEach(cleanup);

// Atlas 128×128 (16×16 Zellen à 8×8), nur das Alpha ist gesetzt
function atlas(fill: (cell: number, x: number, y: number) => boolean) {
  const width = 128;
  const data = new Uint8ClampedArray(width * width * 4);
  for (let y = 0; y < width; y++) {
    for (let x = 0; x < width; x++) {
      const cell = Math.floor(y / 8) * 16 + Math.floor(x / 8);
      if (fill(cell, x % 8, y % 8)) data[(y * width + x) * 4 + 3] = 255;
    }
  }
  return { width, height: width, data };
}

describe('glyphRect', () => {
  it('liefert die Zelle per Zeichencode im 16×16-Raster', () => {
    expect(glyphRect('A')).toEqual({ x: 8, y: 32, width: 8, height: 8 });
    expect(glyphRect('a')).toEqual({ x: 8, y: 48, width: 8, height: 8 });
    expect(glyphRect('0')).toEqual({ x: 0, y: 24, width: 8, height: 8 });
  });

  it('kennt nur ASCII', () => {
    expect(glyphRect('ä')).toBeNull();
    expect(glyphRect('…')).toBeNull();
  });
});

describe('measureGlyphWidths', () => {
  it('Breite = letzte nicht transparente Spalte + 1', () => {
    // 'A' bis Spalte 4, 'i' nur Spalte 0, 'W' bis Spalte 5
    const columns: Record<number, number> = { 65: 4, 105: 0, 87: 5 };
    const widths = measureGlyphWidths(atlas((cell, x) => columns[cell] !== undefined && x <= columns[cell]));
    expect(widths[65]).toBe(5);
    expect(widths[105]).toBe(1);
    expect(widths[87]).toBe(6);
  });

  it('leere Zellen haben Breite 0, das Leerzeichen 3', () => {
    const widths = measureGlyphWidths(atlas(() => false));
    expect(widths[66]).toBe(0);
    expect(widths[32]).toBe(3);
    expect(widths).toHaveLength(256);
  });
});

describe('McText', () => {
  it('ohne Atlas: Rückfallschrift, Text lesbar', () => {
    render(<McText>Board</McText>);
    const text = screen.getByLabelText('Board');
    expect(text.textContent).toBe('Board');
    expect(text.classList.contains('mc-text--fallback')).toBe(true);
    expect(text.querySelector('.mc-glyph')).toBeNull();
  });

  it('mit Atlas: je Zeichen eine Glyphe, Text bleibt für Screenreader und Tests lesbar', () => {
    render(
      <ResourcesProvider urls={{ [FONT_ATLAS]: '/atlas.png' }}>
        <button type="button">
          <McText>Board</McText>
        </button>
      </ResourcesProvider>,
    );
    const text = screen.getByLabelText('Board');
    expect(text.classList.contains('mc-text--fallback')).toBe(false);
    expect(text.textContent).toBe('Board');
    expect(screen.getByRole('button', { name: 'Board' })).toBeTruthy();
    const glyphs = text.querySelectorAll<HTMLElement>('.mc-glyph');
    expect(glyphs).toHaveLength(5);
    // 'B' = 66 → Spalte 2, Zeile 4; Skalierung 2×; ohne Canvas die feste Breite
    expect(glyphs[0].style.backgroundPosition).toBe('-32px -64px');
    expect(glyphs[0].style.width).toBe(`${DEFAULT_GLYPH_WIDTH * 2}px`);
    expect(glyphs[0].closest('[aria-hidden="true"]')).toBeTruthy();
  });

  it('Zeichen außerhalb von ASCII nutzen die Rückfallschrift nur für dieses Zeichen', () => {
    render(
      <ResourcesProvider urls={{ [FONT_ATLAS]: '/atlas.png' }}>
        <McText>Bü</McText>
      </ResourcesProvider>,
    );
    const text = screen.getByLabelText('Bü');
    expect(text.querySelectorAll('.mc-glyph')).toHaveLength(1);
    expect(text.querySelector('.mc-glyph-fallback')?.getAttribute('data-char')).toBe('ü');
    expect(text.textContent).toBe('Bü');
  });
});
