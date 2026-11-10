// 9-Slice-Tabelle der GUI-Sprites und das daraus gebaute border-image (Ränder aus den .mcmeta-Dateien des Spiels)
import { describe, expect, it } from 'vitest';
import { SPRITES, sliceStyle } from '../src/ui/Sprite.js';

describe('sliceStyle', () => {
  it('baut border-image aus Rand × 2', () => {
    expect(sliceStyle('button')).toMatchObject({
      borderImageSource: 'var(--mc-button, none)',
      borderImageSlice: '3 fill',
      borderImageWidth: '6px',
      borderImageRepeat: 'stretch',
    });
    expect(sliceStyle('textField')).toMatchObject({
      borderImageSource: 'var(--mc-text-field, none)',
      borderImageSlice: '1 fill',
      borderImageWidth: '2px',
    });
    expect(SPRITES.popup).toEqual({ width: 236, height: 34, border: 6, variable: '--mc-popup' });
  });

  it('kennt alle Sprites des Themes mit ihren Rändern', () => {
    expect(Object.keys(SPRITES).sort()).toEqual(
      [
        'button',
        'buttonDisabled',
        'buttonHighlighted',
        'popup',
        'textField',
        'textFieldHighlighted',
        'tooltipBackground',
        'tooltipFrame',
      ].sort(),
    );
    expect(SPRITES.button).toEqual({ width: 200, height: 20, border: 3, variable: '--mc-button' });
    expect(SPRITES.textFieldHighlighted).toEqual({ width: 200, height: 20, border: 1, variable: '--mc-text-field-highlighted' });
    expect(SPRITES.tooltipBackground.border).toBe(9);
    expect(SPRITES.tooltipFrame.border).toBe(10);
  });
});
